use core::{num::NonZeroU64, sync::atomic::{AtomicU64, Ordering}};

use alloc::boxed::Box;
use x86_64::{VirtAddr, structures::paging::{FrameDeallocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate, page_table::{PageTableEntry, PageTableLevel}}};

use crate::{allocator::{HeapFrameAllocator, get_frame_allocator}, arch::paging::{get_page_table, physical_memory_offset}, syscalls::TCD};

use self::{elf::Elf, switch::ContextRegs};

pub mod elf;
pub mod init;
pub mod switch;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ProcessId(pub NonZeroU64);

const KERNEL_STACK_SIZE: usize = 64 * 1024;// 64Kb
const USER_STACK_SIZE: usize = 64 * 1024;// 64Kb
const USERSPACE_STACK_ADDR: u64 = 0x4000_0000;
static NEXT_PID: AtomicU64 = AtomicU64::new(2);

pub struct OwnedStack<const SIZE: usize>(pub Box<[u8; SIZE]>);

impl<const SIZE: usize> OwnedStack<SIZE> {
    pub fn alloc_uninit() -> Self {
        let boxed = Box::new_uninit();
        // Safety: it will contained uninitialized data but we don't care since
        // everything is just integers
        OwnedStack(unsafe { boxed.assume_init() })
    }
}



fn allocate_pid() -> ProcessId {
    let raw = NEXT_PID.fetch_add(1, Ordering::SeqCst);
    if raw == u64::MAX {
        panic!("Process space ended!")
    }
    // SAFETY: the first value on NEXT_PID is 1 and we go up
    // the only instance of raw being 0 is if the number wraps (and it should
    // panic if this happens)
    unsafe { ProcessId(NonZeroU64::new_unchecked(raw)) }
}

pub struct TaskContext {
    pub pid: ProcessId,
    pub arch_regs: ContextRegs,
    pub page_table: UserPageTable,
    pub kernel_stack: Option<OwnedStack<KERNEL_STACK_SIZE>>,
    pub user_stack: Option<OwnedStack<USER_STACK_SIZE>>,
    // Runtime data, it should be inside of kernel_stack
    // When switched the kernel stack pointer is saved here
    pub user_entry_point: VirtAddr,
}

impl TaskContext {
    pub unsafe fn create_init() -> Self {
        TaskContext {
            pid: ProcessId(NonZeroU64::new(1).unwrap()),
            arch_regs: ContextRegs::default(),
            page_table: UserPageTable::from_current(),
            kernel_stack: None,
            user_stack: None,
            user_entry_point: VirtAddr::zero(),
        }
    }

    pub fn create(func: extern fn()) -> Self {
        let mut ktable = get_page_table();
        let mut ctx = TaskContext {
            pid: allocate_pid(),
            arch_regs: ContextRegs::default(),
            page_table: UserPageTable::new_from(ktable.level_4_table()),
            kernel_stack: Some(OwnedStack::alloc_uninit()),
            user_stack: Some(OwnedStack::alloc_uninit()),
            user_entry_point: VirtAddr::zero(),
        };

        ctx.arch_regs.reload_cr3(&mut ctx.page_table);

        let kernel_stack = &ctx.kernel_stack.as_ref().unwrap().0;
        // The stack grows back
        ctx.arch_regs.rsp = kernel_stack.as_ptr() as usize + kernel_stack.len();
        // When we switch to this context we will call the code using a "ret" instruction
        // the ret will take an address from the stack and jump to that, so at the top of the stack
        // there should be the initial function
        unsafe {
            ctx.arch_regs.push_stack(func as usize);
        }

        unsafe { ctx.mount_user_stack() };

        ctx
    }

    unsafe fn mount_user_stack(&mut self) {
        let page_table = get_page_table();
        let mut frame_allocator = get_frame_allocator();
        let real_stack = page_table
            .translate_addr(VirtAddr::new(self.user_stack.as_ref().unwrap().0.as_ptr() as *const _ as u64))
            .unwrap();
        // Map stack
        self.page_table.offset_page()
            .map_to(
                Page::containing_address(VirtAddr::new(USERSPACE_STACK_ADDR)),
                PhysFrame::<Size4KiB>::containing_address(real_stack),
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE
                    | PageTableFlags::USER_ACCESSIBLE,
                &mut *frame_allocator,
            )
            .unwrap()
            .flush();
    }

    pub unsafe fn prepare_tcb() {
        TCD.user_stack_pointer = USERSPACE_STACK_ADDR + USER_STACK_SIZE as u64 - 128;
    }

    pub fn load_elf(&mut self, elf: &Elf<'static>) {
        elf.mount_into(&mut self.page_table.offset_page());
        self.user_entry_point = VirtAddr::new(elf.header().e_entry);
    }
}

pub struct UserPageTable {
    page_table: Box<PageTable>,
}

impl UserPageTable {
    pub unsafe fn from_current() -> Self {
        let mut page_table = get_page_table();
        let table = page_table.level_4_table();
        // TODO: this seems like a bad idea, ok it should NEVER be deallocated
        // but it that happens we're in big trouble, the initial page table isn't in heap memory
        UserPageTable {
            page_table: Box::from_raw(table),
        }
    }

    pub fn new_from(table: &PageTable) -> Self {
        let mut new_table = Box::new(PageTable::new());

        // Copy higher-half pages
        for (index, entry) in table.iter().enumerate().skip(256) {
            new_table[index] = entry.clone();// Clone the POINTER, NOT THE WHOLE SUB-TABLE
        }

        UserPageTable {
            page_table: new_table,
        }
    }

    pub fn offset_page(&mut self) -> OffsetPageTable {
        unsafe {
            OffsetPageTable::new(&mut self.page_table, physical_memory_offset())
        }
    }

    pub fn get_frame(&mut self) -> PhysFrame {
        let vaddr = VirtAddr::new(&*self.page_table as *const _ as u64);
        let paddr = self.offset_page().translate_addr(vaddr).unwrap();
        PhysFrame::from_start_address(paddr).unwrap()
    }
}

impl Drop for UserPageTable {
    fn drop(&mut self) {

        unsafe fn unmap_worker(falloc: &mut HeapFrameAllocator, entry: &mut PageTableEntry, level: PageTableLevel) {
            if !entry.flags().contains(PageTableFlags::PRESENT) ||
                entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                return;
            }

            // First deallocate lower levels, then deallocate myself
            if let Some(lower_level) = level.next_lower_level() {
                let offset = physical_memory_offset();
                let table = &mut *(offset + entry.addr().as_u64()).as_mut_ptr() as &mut PageTable;
                for e in table.iter_mut() {
                    unmap_worker(falloc, e, lower_level);
                }
            }

            // It must be present and not huge
            let frame = entry.frame().unwrap();
            falloc.deallocate_frame(frame);
        }

        let mut falloc = get_frame_allocator();
        // Deallocate only the lower entries since the kernel entries are shared between all of the tables
        // (shared, not copied, they literally point to the same subtables)
        for e in self.page_table.iter_mut().take(256) {
            unsafe {
                unmap_worker(&mut falloc, e, PageTableLevel::Four);
            }
        }
        // the last table is the boxed one, and will be dropped after this
    }
}

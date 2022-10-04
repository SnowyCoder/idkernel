use core::{num::NonZeroU64, sync::atomic::{AtomicU64, Ordering}};

use alloc::{boxed::Box, vec::Vec};
use x86_64::{VirtAddr, structures::paging::{Mapper, Page, PageTableFlags, Size4KiB}};

use crate::{allocator::get_frame_allocator, arch::paging::get_page_table, capability::syscall::TaskCapabilityStorage, file::syscall::TaskFileStorage, syscalls::TCD};

use super::{UserPageTable, elf::Elf, switch::ContextRegs};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct TaskId(pub NonZeroU64);

const KERNEL_STACK_SIZE: usize = 16 * 1024;// 64Kb
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



fn allocate_pid() -> TaskId {
    let raw = NEXT_PID.fetch_add(1, Ordering::SeqCst);
    if raw == u64::MAX {
        panic!("Rask id space ended!")
    }
    // SAFETY: the first value on NEXT_PID is 1 and we go up
    // the only instance of raw being 0 is if the number wraps (and it should
    // panic if this happens)
    unsafe { TaskId(NonZeroU64::new_unchecked(raw)) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskState {
    Sleepy,// Sleepy tasks are initial tasks, they do nothing but sleep (like me!)
    User,
    Dying,// Will be killed after switching
}

pub struct TaskContext {
    pub id: TaskId,
    pub parent: Option<TaskId>,
    pub children: Vec<TaskId>,
    pub state: TaskState,
    pub arch_regs: ContextRegs,
    pub capabilities: TaskCapabilityStorage,
    pub files: TaskFileStorage,
    pub page_table: UserPageTable,
    pub kernel_stack: Option<OwnedStack<KERNEL_STACK_SIZE>>,
    pub user_stack: Option<OwnedStack<USER_STACK_SIZE>>,
    // Runtime data, it should be inside of kernel_stack
    // When switched the kernel stack pointer is saved here
    pub user_entry_point: VirtAddr,
}

impl TaskContext {
    pub unsafe fn create_init() -> Self {
        let id = TaskId(NonZeroU64::new(1).unwrap());
        let mut ctx = TaskContext {
            id,
            parent: None,
            children: Vec::new(),
            state: TaskState::Sleepy,
            arch_regs: ContextRegs::default(),
            page_table: UserPageTable::from_current(),
            kernel_stack: None,
            user_stack: None,
            user_entry_point: VirtAddr::zero(),
            capabilities: Default::default(),
            files: TaskFileStorage::new(id),
        };

        ctx.arch_regs.reload_cr3(&mut ctx.page_table);

        ctx
    }

    pub fn create(parent: TaskId, func: extern fn()) -> Self {
        let mut ktable = get_page_table();
        let id = allocate_pid();
        let mut ctx = TaskContext {
            id,
            parent: Some(parent),
            children: Vec::new(),
            state: TaskState::User,
            arch_regs: ContextRegs::default(),
            page_table: UserPageTable::new_from(ktable.level_4_table()),
            kernel_stack: Some(OwnedStack::alloc_uninit()),
            user_stack: Some(OwnedStack::alloc_uninit()),
            user_entry_point: VirtAddr::zero(),
            capabilities: Default::default(),
            files: TaskFileStorage::new(id),
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

        let user_stack = &self.user_stack.as_ref().unwrap().0;
        let stack_start = VirtAddr::new(user_stack.as_ptr() as *const _ as u64);
        let heap_stack_range = Page::<Size4KiB>::range_inclusive(
            Page::containing_address(stack_start),
            Page::containing_address(stack_start + user_stack.len() - 1u64),
        );

        let mut offset_page = self.page_table.offset_page();
        let mut user_page = Page::containing_address(VirtAddr::new(USERSPACE_STACK_ADDR));
        for heap_page in heap_stack_range {
            let real_stack = page_table
                .translate_page(heap_page)
                .unwrap();

            offset_page
                .map_to(
                    user_page,
                    real_stack,
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::NO_EXECUTE
                        | PageTableFlags::USER_ACCESSIBLE,
                    &mut *frame_allocator,
                )
                .unwrap()
                .flush();
                user_page += 1;
        }
    }

    pub unsafe fn prepare_tcb(&self) {
        TCD.user_stack_pointer = USERSPACE_STACK_ADDR + USER_STACK_SIZE as u64 - 128;
    }

    pub fn load_elf(&mut self, elf: &Elf) {
        elf.mount_into(&mut self.page_table.offset_page());
        self.user_entry_point = VirtAddr::new(elf.header().e_entry);
    }
}
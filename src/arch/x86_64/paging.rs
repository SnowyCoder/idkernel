
use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::{structures::paging::PageTable, VirtAddr, PhysAddr};
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size1GiB, Size4KiB};
use bootloader::boot_info::{MemoryRegions, MemoryRegionKind};

use crate::{println};

static PHYSICAL_MEMORY_OFFSET: AtomicU64 = AtomicU64::new(0);

pub fn physical_memory_offset() -> VirtAddr {
    // SAFETY: checked on atomic write
    unsafe { VirtAddr::new_unsafe(PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed)) }
}

pub unsafe fn init_phys_mem_off(addr: u64) {
    PHYSICAL_MEMORY_OFFSET.store(addr, Ordering::SeqCst);
}

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

pub fn get_page_table() -> OffsetPageTable<'static> {

    return unsafe { init(physical_memory_offset()) };
}

pub fn create_lev4_page_table<T: FrameAllocator<Size4KiB>>(offset: VirtAddr, frame_allocator: &mut T) -> OffsetPageTable {
    let frame = frame_allocator.allocate_frame().expect("Cannot allocate frame");
    let addr = offset + frame.start_address().as_u64();
    let ptr = addr.as_mut_ptr();
    let level_4_table = unsafe {
        *ptr = PageTable::new();
        &mut *ptr
    };

    OffsetPageTable::new(level_4_table, phys_offset)
}

fn add_physical_memory_map<T>(table: &mut PageTable, memory_size: u64, framebuffer: &[u8], frame_allocator: &mut T)
        where T: FrameAllocator<Size4KiB> {
    let mut ptable = get_page_table();
    let offset = physical_memory_offset();

    {
        let start_frame = PhysFrame::containing_address(PhysAddr::zero());
        let end_frame: PhysFrame<Size1GiB> = PhysFrame::containing_address(PhysAddr::new(memory_size - 1));


        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset.as_u64()));
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            unsafe {
                ptable.map_to(page, frame, flags, frame_allocator)
                        .expect("Error: cannot map")
                        .ignore();
            }
        }
    }

    let first_addr = framebuffer.as_ptr() as u64;
    let end_addr = first_addr + framebuffer.len() as u64;
    
    println!("Framebuffer: {:x} - {:x}", first_addr, end_addr);
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
                                   -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}


/// Private function that is called by `translate_addr`.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats
/// the whole body of unsafe functions as an unsafe block. This function must
/// only be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
                        -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // read the active level 4 frame from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // traverse the multi-level page table
    for &index in &table_indexes {
        // convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // read the page table entry and update `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // calculate the physical address by adding the page offset
    Some(frame.start_address() + u64::from(addr.page_offset()))
}




pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}
impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions
            .map(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

use x86_64::{
    structures::paging::{
        page_table::{PageTableEntry, PageTableLevel},
        OffsetPageTable, PageTable, PageTableFlags,
    },
    VirtAddr,
};

mod debug;
mod frame_allocator;
mod thread;

pub use debug::{explore_page_ranges, print_tables};
pub use frame_allocator::{
    get_boot_frame_allocator, init_boot_frame_allocator, BootInfoFrameAllocator,
};
pub use thread::setup_thread_data;

use super::consts::KERNEL_PHYSICAL_MEMORY_START;

pub fn physical_memory_offset() -> VirtAddr {
    // SAFETY: checked on const::check_boot_info
    unsafe { VirtAddr::new_unsafe(KERNEL_PHYSICAL_MEMORY_START) }
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

// TODO: use refcell?
pub fn get_page_table() -> OffsetPageTable<'static> {
    return unsafe { init(physical_memory_offset()) };
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Cleans up unused mappings of the bootloader
///
/// When the bootloader calls the kernel everything is mapped correctly
/// in the higher half except some entries (the GDT and the switch function)
/// this function will clear everything mapped on the lower half
///
/// SAFETY: you should call this only when there's nothing useful mapped to
/// the lower half of the virtual addresses of the current page table
/// (so before userspace)
pub unsafe fn fix_bootloader_pollution() {
    let mut table = get_page_table();

    unsafe fn clear_leaf(entry: &mut PageTableEntry, level: PageTableLevel) {
        if !entry.flags().contains(PageTableFlags::PRESENT) {
            return;
        }

        match (entry.flags().contains(PageTableFlags::GLOBAL), level.next_lower_level()) {
            (false, Some(lower_level)) => {
                let offset = physical_memory_offset();
                let table = &mut *(offset + entry.addr().as_u64()).as_mut_ptr() as &mut PageTable;
                for e in table.iter_mut() {
                    clear_leaf(e, lower_level);
                }
            }
            _ => entry.set_flags(entry.flags() - PageTableFlags::PRESENT),
        }
    }

    for x4 in table.level_4_table().iter_mut().take(512 / 2) {
        clear_leaf(x4, PageTableLevel::Four);
    }
}

/// Globalizes all of the kernelspace addresses
///
/// SAFETY: you should call this when no other processor uses this
/// page_table and when there's nothing in userspace
pub unsafe fn globalize_kernelspace() {
    let mut table = get_page_table();

    unsafe fn globalize_leaf(entry: &mut PageTableEntry, level: PageTableLevel) {
        if !entry.flags().contains(PageTableFlags::PRESENT) {
            return;
        }
        if !entry.flags().contains(PageTableFlags::GLOBAL) {
            entry.set_flags(entry.flags() | PageTableFlags::GLOBAL);
        }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            return;
        }
        if let Some(lower_level) = level.next_lower_level() {
            let offset = physical_memory_offset();
            let table = &mut *(offset + entry.addr().as_u64()).as_mut_ptr() as &mut PageTable;
            for e in table.iter_mut() {
                globalize_leaf(e, lower_level);
            }
        }
    }

    for x4 in table.level_4_table().iter_mut().skip(512 / 2) {
        globalize_leaf(x4, PageTableLevel::Four);
    }
}

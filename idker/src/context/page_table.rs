use alloc::boxed::Box;
use x86_64::{VirtAddr, structures::paging::{FrameDeallocator, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Translate, page_table::{PageTableEntry, PageTableLevel}}};

use crate::{allocator::{HeapFrameAllocator, get_frame_allocator}, arch::paging::{get_page_table, physical_memory_offset}};


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
            // if we are a level 2 entry we're pointing to level 1 page tables so there is no need to recurr
            // (we just unmap level 1 tables, but we don't unmap what they point to.)
            if level != PageTableLevel::Two {
                let lower_level = level.next_lower_level().unwrap();// We're not on level 1.
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

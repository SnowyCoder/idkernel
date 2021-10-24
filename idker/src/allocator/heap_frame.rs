use core::num::NonZeroU64;

use alloc::{boxed::Box, vec::Vec};
use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use conquer_once::spin::OnceCell;
use spin::{Mutex, MutexGuard};
use x86_64::{
    structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB, Translate},
    PhysAddr, VirtAddr,
};

use crate::arch::paging::get_page_table;

static HEAP_FRAME_ALLOCATOR: OnceCell<Mutex<HeapFrameAllocator>> = OnceCell::uninit();

pub fn get_frame_allocator() -> MutexGuard<'static, HeapFrameAllocator> {
    HEAP_FRAME_ALLOCATOR
        .get()
        .expect("Heap frame allocator not initialized")
        .lock()
}

pub unsafe fn init_frame_allocator(regions: &MemoryRegions) {
    HEAP_FRAME_ALLOCATOR.init_once(|| Mutex::new(HeapFrameAllocator::new(regions)))
}

#[derive(Debug)]
struct HeapRegion {
    start: u64,
    end: u64,
    virt_start: Option<NonZeroU64>,
}

pub struct HeapFrameAllocator {
    entries: Vec<HeapRegion>,
}

impl HeapFrameAllocator {
    pub fn new(memory_map: &MemoryRegions) -> Self {
        let entries = memory_map
            .iter()
            .filter(|x| x.kind == MemoryRegionKind::Usable)
            .filter(|x| x.end - x.start >= 4096)
            .map(|x| HeapRegion {
                start: x.start,
                end: x.end,
                virt_start: None,
            })
            .collect();
        HeapFrameAllocator { entries }
    }

    fn check_region_virt(&mut self, paddr: PhysAddr, vaddr: VirtAddr) {
        let addr = paddr.as_u64();
        let region = self
            .entries
            .iter_mut()
            .find(|x| addr >= x.start && addr < x.end)
            .expect("Cannot find allocated page region!");

        if region.virt_start.is_none() {
            let addr = vaddr.as_u64() - (paddr - region.start).as_u64();
            region.virt_start = Some(NonZeroU64::new(addr).unwrap())
        }
    }

    fn phys_to_virt(&self, paddr: PhysAddr) -> VirtAddr {
        let addr = paddr.as_u64();
        let entry = self
            .entries
            .iter()
            .find(|x| addr >= x.start && addr < x.end)
            .expect("Cannot find entry for page");

        let vstart = entry
            .virt_start
            .expect("Virtual address for region not present yet");

        return VirtAddr::new(vstart.get() + (paddr.as_u64() - entry.start));
    }
}

#[repr(C, align(4096))]
struct StupidPage {
    data: [u8; 4096],
}

// SAFETY: using the Heap allocator to allocate a frame will always return a currently unused frame
unsafe impl FrameAllocator<Size4KiB> for HeapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let boxed = Box::<StupidPage>::try_new_uninit().ok()?;
        let addr = Box::into_raw(boxed);
        let vaddr = VirtAddr::from_ptr(addr);
        let paddr = get_page_table().translate_addr(vaddr).unwrap();

        self.check_region_virt(paddr, vaddr);

        Some(PhysFrame::from_start_address(paddr).unwrap())
    }
}

impl FrameDeallocator<Size4KiB> for HeapFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        let paddr = frame.start_address();
        let vaddr = self.phys_to_virt(paddr);
        let boxed = Box::<StupidPage>::from_raw(vaddr.as_mut_ptr());
        drop(boxed);
    }
}

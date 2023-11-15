use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use conquer_once::spin::OnceCell;
use spin::{Mutex, MutexGuard};
use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};

static FRAME_ALLOCATOR: OnceCell<Mutex<BootInfoFrameAllocator>> = OnceCell::uninit();

pub fn get_boot_frame_allocator() -> MutexGuard<'static, BootInfoFrameAllocator> {
    FRAME_ALLOCATOR
        .get()
        .expect("Frame allocator not initialized")
        .lock()
}

pub unsafe fn init_boot_frame_allocator(regions: &'static MemoryRegions) {
    FRAME_ALLOCATOR.init_once(|| Mutex::new(BootInfoFrameAllocator::init(regions)))
}

type BootInfoIter = impl Iterator<Item = PhysFrame>;

/// Returns an iterator over the usable frames specified in the memory map.
fn usable_frames(memory_map: &'static [MemoryRegion]) -> BootInfoIter {
    // get usable regions from memory map
    let regions = memory_map.iter();
    let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
    // map each region to its address range
    let addr_ranges = usable_regions.map(|r| r.start..r.end);
    // transform to an iterator of frame start addresses
    let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
    // create `PhysFrame` types from the start addresses
    frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
}

pub struct BootInfoFrameAllocator {
    iter: BootInfoIter,
}
impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            iter: usable_frames(memory_map),
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.iter.next()
    }
}

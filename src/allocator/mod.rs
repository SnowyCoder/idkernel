use crate::{
    allocator::heap_frame::init_frame_allocator,
    arch::{
        consts::KERNEL_HEAP_START,
        paging::{get_boot_frame_allocator, get_page_table},
    },
    memory::MemorySize,
    println,
};
use bootloader::boot_info::MemoryRegions;
use fixed_size_block::FixedSizeBlockAllocator;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub mod fixed_size_block;
mod heap_frame;
pub mod linked_list;

pub use heap_frame::{get_frame_allocator, HeapFrameAllocator};

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

pub fn init_heap(regions: &MemoryRegions) {
    let mut boot_frame_alloc = get_boot_frame_allocator();
    let mut mapper = get_page_table();
    let heap_start = Page::containing_address(VirtAddr::new(KERNEL_HEAP_START as u64));

    let mut page = heap_start;
    let mut allocated = 0 as u64;

    let mut allocate_one = move || -> Result<(), MapToError<Size4KiB>> {
        let frame = boot_frame_alloc
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper
                .map_to(page, frame, flags, &mut *boot_frame_alloc)?
                .flush();
        };
        page += 1;
        Ok(())
    };

    loop {
        match allocate_one() {
            Ok(_) => allocated += 1,
            Err(_) => break,
        }
    }

    println!(
        "Heap allocated: {}",
        MemorySize((allocated * Size4KiB::SIZE) as usize)
    );

    unsafe {
        ALLOCATOR
            .lock()
            .init(KERNEL_HEAP_START, allocated * Size4KiB::SIZE);
        init_frame_allocator(regions);
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

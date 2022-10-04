use core::convert::{TryFrom, TryInto};

use crate::{capability::CapabilityType, arch::paging::get_page_table, allocator::get_frame_allocator, context::current_task};

use super::{SyscallResult, SyscallError, check_addr_userspace};

use bitflags::bitflags;
use x86_64::{structures::paging::{Mapper, FrameAllocator, PageTableFlags, Page, Size4KiB, mapper::MapToError, page::PageRange, PhysFrame}, VirtAddr, PhysAddr};


bitflags! {
    pub struct MemoryPerms: u8 {
        const READ = 0x1;
        const WRITE = 0x2;
        const EXECUTE = 0x4;
    }
}

impl TryFrom<MemoryPerms> for PageTableFlags {
    type Error = SyscallError;

    fn try_from(value: MemoryPerms) -> Result<Self, Self::Error> {
        if !value.contains(MemoryPerms::READ) {
            return Err(SyscallError::WrongParameters);
        }
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if value.contains(MemoryPerms::WRITE) {
            flags.insert(PageTableFlags::WRITABLE);
        }
        if !value.contains(MemoryPerms::EXECUTE) {
            flags.insert(PageTableFlags::NO_EXECUTE);
        }
        Ok(flags)
    }
}

fn parse_page_range(at: usize, len: usize) -> SyscallResult<PageRange> {
    // Check userspace
    check_addr_userspace(at)?;
    check_addr_userspace(at + len - 1)?;
    // Check page aligment
    let at = VirtAddr::new(at as u64);
    let start_page = Page::<Size4KiB>::from_start_address(at)
            .map_err(|_| SyscallError::WrongParameters)?;

    let end_page = Page::<Size4KiB>::from_start_address(at + len)
        .map_err(|_| SyscallError::WrongParameters)?;

    Ok(Page::range(start_page, end_page))
}

pub fn map_virt(at: usize, len: usize, perms: usize) -> SyscallResult<()> {
    let page_range = parse_page_range(at, len)?;

    // Check capability
    let task_guard = current_task();
    let task = task_guard.read();
    task.capabilities.handles.iter()
            .find(|x| x.1.ctype == CapabilityType::MapVirtualToRam)
            .ok_or(SyscallError::WrongCapability)?;

    let perms = MemoryPerms::from_bits_truncate(perms as u8);
    let flags = PageTableFlags::try_from(perms)? | PageTableFlags::BIT_10;
    let parent_table_flags = PageTableFlags::PRESENT |
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

    // TODO: Should the operation be atomic?
    // TODO: what about multiple threads?
    // Optimistic behaviour: first allocate assuming the paging is free, then try to map

    let mut frame_allocator = get_frame_allocator();
    // TODO: can we use map_to_with_table_flags? what parent flags should we use?
    // what if it's already mapped?
    for page in page_range {
        let frame = frame_allocator.allocate_frame().ok_or(SyscallError::NoMemory)?;
        let mut table = get_page_table();

        unsafe {
            match table.map_to_with_table_flags(page, frame, flags, parent_table_flags, &mut *frame_allocator) {
                Ok(_) => {},
                Err(MapToError::PageAlreadyMapped(_) | MapToError::ParentEntryHugePage) => return Err(SyscallError::MemoryAlreadyMapped),
                Err(MapToError::FrameAllocationFailed) => return Err(SyscallError::NoMemory)
            };
        }
    }

    Ok(())
}

pub fn map_phys(virt_at: usize, virt_len: usize, perms: usize, phys_at: usize) -> SyscallResult<()> {
    let page_range = parse_page_range(virt_at, virt_len)?;
    let phys_from = PhysAddr::new_truncate(phys_at as u64);
    let phys_to = phys_from + virt_len - 1usize;

    let frame = PhysFrame::from_start_address(phys_from)
            .map_err(|_| SyscallError::WrongParameters)?;
    let end_frame = PhysFrame::from_start_address(phys_to + 1usize)
            .map_err(|_| SyscallError::WrongParameters)?;
    let frame_range = PhysFrame::range(frame, end_frame);

    // Check capability
    let task_guard = current_task();
    let task = task_guard.read();
    task.capabilities.handles.iter()
            .find(|x| {
                return match x.1.ctype {
                    CapabilityType::MapPhysical(f, t) => f <= phys_from && t >= phys_to,
                    _ => false,
                }
            })
            .ok_or(SyscallError::WrongCapability)?;

    let perms = MemoryPerms::from_bits_truncate(perms as u8);
    let flags = perms.try_into()?;
    let parent_table_flags = PageTableFlags::PRESENT |
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

    // TODO: Should the operation be atomic?
    // TODO: what about multiple threads?

    let mut frame_allocator = get_frame_allocator();
    // TODO: can we use map_to_with_table_flags? what parent flags should we use?
    // what if it's already mapped?
    for (page, frame) in page_range.zip(frame_range) {
        let mut table = get_page_table();

        unsafe {
            match table.map_to_with_table_flags(page, frame, flags, parent_table_flags, &mut *frame_allocator) {
                Ok(_) => {},
                Err(MapToError::PageAlreadyMapped(_) | MapToError::ParentEntryHugePage) => return Err(SyscallError::MemoryAlreadyMapped),
                Err(MapToError::FrameAllocationFailed) => return Err(SyscallError::NoMemory)
            };
        }
    }

    Ok(())
}
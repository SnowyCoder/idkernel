use core::{cmp::{max, min}, convert::TryInto, intrinsics::copy_nonoverlapping, slice};

use goblin::{elf64::{header::{ELFMAG, SELFMAG, Header, SIZEOF_EHDR}, program_header::{PF_R, PF_W, PF_X, PT_LOAD, ProgramHeader}}};

use x86_64::{VirtAddr, structures::paging::{OffsetPageTable, FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB}};
use crate::{allocator::get_frame_allocator, arch::paging::physical_memory_offset};



pub struct Elf<'a> {
    data: &'a [u8],
}

impl<'a> Elf<'a> {
    pub fn new(data: &'a [u8]) -> Result<Elf<'a>, &'static str> {
        if data.len() <= SIZEOF_EHDR {
            return Err("Invalid elf size")
        }
        let elf = Elf {
            data
        };
        let header = elf.header();
        if &header.e_ident[..SELFMAG] != ELFMAG {
            return Err("Invalid ELF magic, are you sure this is an elf file?")
        }
        match (header.e_phoff as usize).checked_add(header.e_phnum as usize * core::mem::size_of::<ProgramHeader>()) {
            Some(x) if (x as usize) < data.len() => {},
            _ => return Err("Invalid program headers")
        }
        Ok(elf)
    }

    pub fn header(&self) -> &Header {
        let header_ref = self.data[..SIZEOF_EHDR].try_into().unwrap();
        Header::from_bytes(header_ref)
    }

    pub fn program_headers(&self) -> &[ProgramHeader] {
        let header = self.header();
        unsafe {
            // Safety: checked in new
            slice::from_raw_parts(
                self.data.as_ptr().offset(header.e_phoff as isize) as *const ProgramHeader,
                header.e_phnum as usize
            )
        }
    }

    pub fn programs(&'a self) -> impl Iterator<Item=(&'a ProgramHeader, &'a [u8])> + 'a {
        self.program_headers().iter().map(move |header| {
            let d = &self.data[header.p_offset as usize..][..header.p_filesz as usize];
            (header, d)
        })
    }

    pub fn mount_into(&self, table: &mut OffsetPageTable) {
        let mut allocator = get_frame_allocator();
        let offset = physical_memory_offset();

        let headers = self.programs()
                .filter(|(x, _)| x.p_type == PT_LOAD)
                .filter(|(x, _)| x.p_memsz > 0);
        for (header, data) in headers {
            let from = VirtAddr::new(header.p_vaddr);
            let to = from + header.p_memsz;
            check_addr_userspace(from).unwrap();
            check_addr_userspace(to).unwrap();
            let from_page = Page::containing_address(from);
            let to_page = Page::containing_address(to - 1usize);
            let range = Page::<Size4KiB>::range_inclusive(from_page, to_page);

            let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if header.p_flags & PF_X == 0 { flags |= PageTableFlags::NO_EXECUTE; }
            if header.p_flags & PF_W != 0 { flags |= PageTableFlags::WRITABLE; }
            if header.p_flags & PF_R == 0 { panic!("Program section not readable?") }
            let flags = flags;// not mutable anymore

            // index into the data array
            let file_index = from_page.start_address().as_u64() as isize - from.as_u64() as isize;
            for page in range {
                let frame = allocator.allocate_frame().expect("Cannot get frame for init");

                unsafe {
                    table.map_to(page, frame, flags, &mut *allocator)
                            .expect("Failed to map program")
                            .flush();

                    let ptr = (offset + frame.start_address().as_u64()).as_mut_ptr() as *mut [u8; Size4KiB::SIZE as usize];
                    ptr.write([0; Size4KiB::SIZE as usize]);

                    let data_start = max(file_index, 0);
                    let data_end = min(file_index + Size4KiB::SIZE as isize, data.len() as isize);

                    // offset to the page,
                    let off = max(-file_index, 0);
                    if data_start <= data_end {
                        copy_nonoverlapping(
                            data.as_ptr().offset(data_start),
                            (offset + frame.start_address().as_u64()).as_mut_ptr::<u8>().offset(off),
                            (data_end - data_start) as usize
                        );
                    }
                }
            }
        }
    }
}

fn check_addr_userspace(addr: VirtAddr) -> Result<(), &'static str> {
    return match addr.as_u64() & 1 << (u64::BITS - 1) {
        0 => Ok(()),
        _ => Err("Address is in kernel space"),
    }
}
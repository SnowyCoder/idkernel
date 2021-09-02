use x86_64::VirtAddr;
use acpi::{AcpiTables, PhysicalMapping};
use core::ptr::NonNull;
use core::mem;
use x86_64::structures::paging::{Size4KiB, FrameAllocator, Mapper};
use crate::{arch::x86_64::apic::LOCAL_APIC, println};

pub fn init(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_mem_offset: VirtAddr
) {
    let handler = CustomAcpiHandler(phys_mem_offset);

    let tables = unsafe { AcpiTables::search_for_rsdp_bios(handler) }
        .expect("Unable to parse ACPI tables");

    println!("rev: {}", tables.revision);
    let platform_info = tables.platform_info().expect("Unable to parse platform info");


    println!("int: {:?}", platform_info.interrupt_model);
    match platform_info.interrupt_model {
        acpi::InterruptModel::Apic(apic) => {
            let lapic = unsafe { &mut LOCAL_APIC };
            unsafe {
                println!("APIC BASE real: {}", apic.local_apic_address);
                lapic.init(VirtAddr::new(phys_mem_offset.as_u64() + apic.local_apic_address))
                    .unwrap();
            };
        },
        _ => panic!("Unknown interrupt model!"),
    }
    
    println!("pow: {:?}", platform_info.power_profile);
    if let Some(proc) = platform_info.processor_info {
        println!("Boot: {:#?}", proc.boot_processor);
        println!("Apps: {:#?}", proc.application_processors);

        #[cfg(feature = "multi_core")]
            super::multi_core::init(frame_allocator, &proc);
    } else {
        println!("proc: None");
    }

}

#[derive(Clone, Copy)]
struct CustomAcpiHandler(VirtAddr);

impl acpi::AcpiHandler for CustomAcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new((self.0 + physical_address as usize).as_mut_ptr::<T>()).unwrap(),
            region_length: mem::size_of::<T>(),
            mapped_length: mem::size_of::<T>(),
            handler: self.clone()
        }
    }

    fn unmap_physical_region<T>(&self, region: &PhysicalMapping<Self, T>) {
    }
}

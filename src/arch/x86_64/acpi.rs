use crate::{
    arch::{paging::physical_memory_offset, x86_64::apic::LOCAL_APIC},
    print, println,
};
use acpi::{AcpiTables, PhysicalMapping};
use core::{mem, ptr::NonNull};
use x86_64::VirtAddr;

pub fn init(rsdp_addr: u64) {
    let phys_mem_offset = physical_memory_offset();
    let handler = CustomAcpiHandler(phys_mem_offset);

    let tables = unsafe { AcpiTables::from_rsdp(handler, rsdp_addr as usize) }
        .expect("Unable to parse ACPI tables");

    println!("rev: {}", tables.revision);
    let platform_info = tables
        .platform_info()
        .expect("Unable to parse platform info");

    //println!("int: {:?}", platform_info.interrupt_model);
    match platform_info.interrupt_model {
        acpi::InterruptModel::Apic(apic) => {
            let lapic = unsafe { &mut LOCAL_APIC };
            unsafe {
                println!("APIC BASE real: {}", apic.local_apic_address);
                lapic
                    .init(VirtAddr::new(
                        phys_mem_offset.as_u64() + apic.local_apic_address,
                    ))
                    .unwrap();
            };
        }
        _ => panic!("Unknown interrupt model!"),
    }

    println!("pow: {:?}", platform_info.power_profile);
    if let Some(proc) = platform_info.processor_info {
        print!("Processors: [{:#?}]", proc.boot_processor.local_apic_id);
        for x in proc.application_processors.iter() {
            print!(" {}", x.local_apic_id);
        }
        println!();

        #[cfg(feature = "multi_core")]
        super::multi_core::init(&proc);
    } else {
        println!("proc: None");
    }
}

#[derive(Clone, Copy)]
struct CustomAcpiHandler(VirtAddr);

impl acpi::AcpiHandler for CustomAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        _size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping::new(
            physical_address,
            NonNull::new((self.0 + physical_address as usize).as_mut_ptr::<T>()).unwrap(),
            mem::size_of::<T>(),
            mem::size_of::<T>(),
            self.clone(),
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

use crate::{
    allocator::{get_frame_allocator, HeapFrameAllocator},
    arch::x86_64::{
        apic::LOCAL_APIC,
        paging::{get_page_table, physical_memory_offset},
    },
    hlt_loop, print, println,
};
use acpi::platform::{Processor, ProcessorInfo, ProcessorState};
use alloc::boxed::Box;
use core::{
    intrinsics::{atomic_load_seqcst, atomic_store_seqcst},
    sync::atomic::{AtomicBool, Ordering},
};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

const TRAMPOLINE_ADDR: u64 = 0x8000;
static TRAMPOLINE_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));
static AP_READY: AtomicBool = AtomicBool::new(false);
static BSP_READY: AtomicBool = AtomicBool::new(false);

pub fn init_ap_processor(p: &Processor) {
    println!("Starting: AP {}", p.processor_uid);

    // TODO: put a non-empty page after the stack to catch stack overflows
    const STACK_SIZE: u64 = 128 * 1024;
    type Stack = [u8; STACK_SIZE as usize];
    let stack = Box::<Stack>::new_uninit();
    let stack = Box::leak(stack);

    let (level_4_table_frame, _) = Cr3::read();

    unsafe {
        Trampoline::setup(
            p.local_apic_id as u64,
            level_4_table_frame.start_address().as_u64(),
            stack.as_ptr() as u64,
            stack.as_ptr() as u64 + STACK_SIZE,
        );
    }
    println!("| Setup done");
    AP_READY.store(false, Ordering::SeqCst);

    let lapic = unsafe { &mut LOCAL_APIC };

    println!("| Sending init IPI... (from: {})", lapic.id());
    // Send INIT IPI
    {
        let mut icr = 0x4500;
        if lapic.is_ver2 {
            icr |= (p.local_apic_id as u64) << 32;
        } else {
            icr |= (p.local_apic_id as u64) << 56;
        }
        lapic.set_icr(icr);
    }

    println!("| Sending start IPI...");
    // Send START IPI
    {
        let ap_segment = (TRAMPOLINE_ADDR >> 12) & 0xFF;
        let mut icr = 0x4600 | ap_segment as u64;
        if lapic.is_ver2 {
            icr |= (p.local_apic_id as u64) << 32;
        } else {
            icr |= (p.local_apic_id as u64) << 56;
        }
        lapic.set_icr(icr);
    }

    println!("| Trampoline...");
    // Wait for trampoline
    unsafe {
        while !Trampoline::is_ready() {
            print!(".");
            //core::arch::x86_64::_mm_pause();
        }
    }

    println!(" Setup...");
    while !AP_READY.load(Ordering::SeqCst) {
        unsafe { core::arch::x86_64::_mm_pause() };
    }
    println!("AP {} READY!", p.processor_uid);
}

pub fn init(proc_info: &ProcessorInfo) {
    println!("Initializing multicore");

    println!("Writing trampoline...");
    {
        let mut frame_allocator = get_frame_allocator();
        unsafe { Trampoline::write(&mut (*frame_allocator)) };
    };

    proc_info
        .application_processors
        .iter()
        .filter(|p| p.state != ProcessorState::Disabled)
        .for_each(|p| init_ap_processor(p));

    println!("Unwriting trampoline...");
    unsafe {
        Trampoline::unwrite();
    }
}

struct Trampoline;

impl Trampoline {
    unsafe fn write(frame_allocator: &mut HeapFrameAllocator) {
        let dest = VirtAddr::new(TRAMPOLINE_ADDR);
        assert!(TRAMPOLINE_DATA.len() < 4096);

        let mut page_table = get_page_table();
        page_table
            .identity_map(
                PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(TRAMPOLINE_ADDR)),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                frame_allocator,
            )
            .unwrap()
            .flush();

        for i in 0..TRAMPOLINE_DATA.len() {
            atomic_store_seqcst((dest.as_u64() as *mut u8).add(i), TRAMPOLINE_DATA[i]);
        }
    }

    unsafe fn setup(cpu_id: u64, page_table: u64, stack_start: u64, stack_end: u64) {
        let ap_ready = (TRAMPOLINE_ADDR + physical_memory_offset().as_u64() + 8) as *mut u64;
        let ap_cpu_id = ap_ready.offset(1);
        let ap_page_table = ap_ready.offset(2);
        let ap_stack_start = ap_ready.offset(3);
        let ap_stack_end = ap_ready.offset(4);
        let ap_code = ap_ready.offset(5);

        atomic_store_seqcst(ap_ready, 0);
        atomic_store_seqcst(ap_cpu_id, cpu_id);
        atomic_store_seqcst(ap_page_table, page_table);
        atomic_store_seqcst(ap_stack_start, stack_start);
        atomic_store_seqcst(ap_stack_end, stack_end);
        atomic_store_seqcst(ap_code, kstart_ap as u64);
    }

    unsafe fn is_ready() -> bool {
        let ap_ready = (TRAMPOLINE_ADDR + 8 + physical_memory_offset().as_u64()) as *mut u64;
        atomic_load_seqcst(ap_ready) != 0
    }

    unsafe fn unwrite() {
        let mut page_table = get_page_table();
        let (_frame, flush) = page_table
            .unmap(Page::<Size4KiB>::containing_address(VirtAddr::new(
                TRAMPOLINE_ADDR,
            )))
            .unwrap();
        flush.flush();
    }
}

#[repr(C)]
pub struct KernelArgsAp {
    cpu_id: u64,
    page_table: u64,
    stack_start: u64,
    stack_end: u64,
}

pub unsafe extern "C" fn kstart_ap(args_ptr: *const KernelArgsAp) -> ! {
    use crate::{gdt, interrupts};

    let args = &*args_ptr;

    interrupts::init_idt();

    {
        let mut frame_allocator = get_frame_allocator();
        crate::arch::x86_64::paging::setup_thread_data(args.cpu_id, &mut *frame_allocator);
    }
    gdt::init();

    println!("READY: {}", args.cpu_id);

    AP_READY.store(true, Ordering::SeqCst);

    while !BSP_READY.load(Ordering::SeqCst) {
        core::arch::x86_64::_mm_pause();
    }
    println!("START! {}", args.cpu_id);

    hlt_loop();
}

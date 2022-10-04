#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kerneltest::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use alloc::sync::Arc;
use bootloader::{entry_point, BootInfo};

use kerneltest::{allocator, allocator::get_frame_allocator, arch::{
        consts::check_boot_info,
        paging::{
            self, fix_bootloader_pollution,
            globalize_kernelspace,
        },
    }, context::{TaskContext, set_current_task_id, switch_to_next_task, tasks_mut}, file::InitFsFolderHandle, gdt, println, syscalls::{self, start_initproc}, vga_framebuffer::init_vga_framebuffer};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    check_boot_info(boot_info);
    let fb = boot_info
        .framebuffer
        .as_mut()
        .expect("No Framebuffer found");
    unsafe { paging::init_boot_frame_allocator(&boot_info.memory_regions) };

    init_vga_framebuffer(fb);
    // Now we can write to screen

    println!("Setting up GDT");

    kerneltest::init();

    unsafe {
        // Remove bootloader-related mappings in the lower-half
        fix_bootloader_pollution();
        // Add global bits in the higher half (yep, no meltdown/spectre mitigations for now)
        globalize_kernelspace();
    }
    allocator::init_heap(&boot_info.memory_regions);

    {
        let mut frame_allocator = get_frame_allocator();

        println!("Setting up thread data");
        unsafe {
            paging::setup_thread_data(0, &mut frame_allocator);
        }
    }
    println!("Setting up GDT");
    gdt::init();

    #[cfg(test)]
    test_main();

    println!("Memory mapping:");
    for x in boot_info.memory_regions.iter() {
        println!("-{:#012x}-{:#012x} {:?}", x.start, x.end, x.kind);
    }

    kerneltest::arch::acpi::init(boot_info.rsdp_addr.into_option().expect("Cannot find rsdp"));
    syscalls::setup_syscalls();

    run_executor()
}

fn run_executor() -> ! {
    unsafe {
        // Init task = main kernel task (with no user thread attached)
        let init = TaskContext::create_init();
        let init_id = init.id;
        tasks_mut().add(init);
        set_current_task_id(init_id);

        // initproc task = a normal task used to run the init process
        let ctx = TaskContext::create(init_id, start_initproc);
        let ctx_id = ctx.id;
        {// Add init file system
            let mut root = ctx.files.root.write();
            root.mount("init", Arc::new(InitFsFolderHandle::from_init_dir("init".into())));
        }
        let mut tasks = tasks_mut();
        tasks.add(ctx);
        tasks.queue_for_execution(ctx_id);
    }
    loop {
        if !switch_to_next_task() {
            x86_64::instructions::interrupts::enable_and_hlt();
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use kerneltest::hlt_loop;

    println!("{}", info);
    hlt_loop()
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kerneltest::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

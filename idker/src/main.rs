#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kerneltest::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use bootloader::{entry_point, BootInfo};

use kerneltest::{allocator, allocator::get_frame_allocator, arch::{
        consts::check_boot_info,
        paging::{
            self, explore_page_ranges, fix_bootloader_pollution, get_page_table,
            globalize_kernelspace,
        },
    }, gdt, println, syscalls, task::{
        executor::{Executor, Spawner},
        keyboard, Task,
    }, utils::shortflags::ShortFlags, vga_framebuffer::init_vga_framebuffer};
use x86_64::structures::paging::Translate;

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

    let mut executor = Executor::new();
    let spawner = executor.spawner().clone();
    executor.spawn(Task::new(example_task(spawner)));
    //executor.spawn(Task::new(print_tables()));
    executor.spawn(Task::new(tuserspace()));
    //executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run_sync()
}

async fn async_number() -> u32 {
    42
}

async fn example_task(spawner: Spawner) {
    let num = async_number().await;
    println!("async number: {}", num);
    spawner.spawn(keyboard::print_keypresses());
}

async fn print_tables() {
    let table = get_page_table();

    let translate = |from| (&table).translate_addr(from).unwrap().as_u64();

    for (from, to, flags) in explore_page_ranges() {
        println!(
            "{:#018x}-{:#018x} -> {:#012x} {}",
            from,
            to,
            translate(from),
            ShortFlags(flags)
        );
    }
}

async fn tuserspace() {
    unsafe {
        println!("Preparing userspace!");
        let addr = syscalls::test_prepare_userspace();
        print_tables().await;
        println!("Entering userspace!");
        syscalls::enter_userspace(addr);
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

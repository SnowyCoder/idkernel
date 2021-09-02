#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kerneltest::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use bootloader::{BootInfo, entry_point};

use kerneltest::arch::x86_64::paging;
use kerneltest::{println, allocator};
use kerneltest::task::{executor::{Executor, Spawner}, keyboard, Task};
use x86_64::VirtAddr;
use kerneltest::vga_framebuffer::init_vga_framebuffer;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let boot_info_addr = VirtAddr::new(boot_info as *const _ as u64);
    let fb = boot_info.framebuffer.as_mut().expect("No Framebuffer found");

    init_vga_framebuffer(fb);

    let phisycal_mem_offset = boot_info.physical_memory_offset.into_option()
        .expect("Physical memory info not found");
    kerneltest::init(phisycal_mem_offset);

    let phys_mem_offset = VirtAddr::new(phisycal_mem_offset);
    let mut mapper = paging::get_page_table();
    let mut frame_allocator = unsafe { paging::BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initialization failed");

    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    kerneltest::arch::x86_64::acpi::init(&mut mapper, &mut frame_allocator, phys_mem_offset);
    for x in boot_info.memory_regions.iter() {
        println!("{:?}", x);
    }

    let mut executor = Executor::new();
    let spawner = executor.spawner().clone();
    executor.spawn(Task::new(example_task(spawner)));
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

#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(const_mut_refs)]
#![feature(core_intrinsics)]
#![feature(custom_test_frameworks)]
#![feature(new_uninit)]
#![feature(thread_local)]
#![feature(type_alias_impl_trait)]
#![feature(generators)]
#![feature(generator_trait)]
#![cfg_attr(test, no_main)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

#[cfg(test)]
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

pub mod allocator;
pub mod arch;
pub mod gdt;
pub mod geniter;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;
pub mod vga_framebuffer;

pub use geniter::GenIter;

#[cfg(test)]
entry_point!(test_kernel_main);

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init();
    test_main();
    hlt_loop();
}

pub fn init() {
    gdt::init_prepaging();
    interrupts::init_idt();
    unsafe {
        let mut pic = interrupts::PICS.lock();
        pic.initialize();
    };
    x86_64::instructions::interrupts::enable();
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

// Simple loop not recognized by the compiler optimizators
// this is really easy to unlock with gdb, just type
// `set $pc += 2` and it'll return to your code
// ( why can't I get breakpoints to work :( )
#[inline(always)]
pub fn gdb_loop() {
    unsafe {
        asm!("2:", "jmp 2b");
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#![feature(lang_items)]

#![feature(start)]
#![no_std]
#![no_main]

use core::{panic::PanicInfo};
use syscall::syscall as s;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    s::yield_();

    // Lol, right, we don't have println
    //println!("My pid: {}", s::Process::my_pid());
    // Exit
    s::exit();
}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler]
fn panic_handler(_x: &PanicInfo) -> ! { loop {} }

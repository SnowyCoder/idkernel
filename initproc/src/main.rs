#![feature(lang_items)]

#![feature(start)]
#![feature(asm)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

// is this possible?
// create_syscall(name, sel, args, rets)


pub unsafe fn syscall_raw6(sel: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let res;
    asm! {
        "syscall",
        in("rax") sel,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        in("r10") d,
        in("r8") e,
        in("r9") f,
        lateout("rax") res
    };
    res
}


#[no_mangle]
pub extern "C" fn _start() -> ! {
    /*unsafe { asm! {
        "mov rax, 1",
        "mov rdi, 2",
        "mov rsi, 3",
        "mov rdx, 4",
        "mov r10, 5",
        "mov r8, 6",
        "mov r9, 7",
        "syscall",
        "2: jmp 2b",
    } };*/
    unsafe { syscall_raw6(1, 2, 3, 4, 5, 6, 7) };
    loop {};
}

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler]
fn panic_handler(_x: &PanicInfo) -> ! { loop {} }

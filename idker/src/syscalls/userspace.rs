use x86_64::{VirtAddr, registers::rflags::RFlags};
use memoffset::offset_of;

use crate::{context::{TaskContext, elf::Elf, init::INIT_DIR}, println, syscalls::{self, TCD, ThreadControlData}};



pub unsafe fn enter_userspace(ip: VirtAddr) -> ! {
    let ip = ip.as_u64();
    let rflags = RFlags::INTERRUPT_FLAG.bits();

    asm!(
        //"2: jmp 2b",
        "mov fs:[{tcd}@tpoff+{ksp_offset}+8], rsp",// save stack pointer
        "mov rsp, fs:[{tcd}@tpoff+{sp_offset}+8]",// load user stack pointer
        "swapgs",
        "sysretq",
        tcd = sym TCD,
        ksp_offset = const offset_of!(ThreadControlData, kernel_stack_pointer),
        sp_offset = const offset_of!(ThreadControlData, user_stack_pointer),
        in("rcx") ip,
        in("r11") rflags,
        options(noreturn)
    )
}

pub unsafe fn test_prepare_userspace() -> TaskContext {
    let mut ctx = TaskContext::create(test_userspace_code);

    let init_proc = match INIT_DIR.entry("initproc").expect("Cannot find initproc") {
        include_dir::InitDirEntry::File(x) => x,
        _ => panic!("initproc is not a file")
    };
    let elf = Elf::new(init_proc).expect("Wrong elf in init data");
    ctx.load_elf(&elf);
    ctx
}

extern "C" fn test_userspace_code() {
    println!("Hello, i'm the new task!");

    //println!("Entering userspace!");
    //syscalls::enter_userspace(ctx.entry_point);
    loop {}
}
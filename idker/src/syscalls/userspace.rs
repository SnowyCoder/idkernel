use x86_64::{VirtAddr, registers::rflags::RFlags};
use memoffset::offset_of;

use crate::{context::{current_task, elf::Elf, init::INIT_DIR}, syscalls::{TCD, ThreadControlData}};



pub unsafe fn enter_userspace(ip: VirtAddr) -> ! {
    let ip = ip.as_u64();
    let rflags = RFlags::INTERRUPT_FLAG.bits();

    asm!(
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

pub extern "C" fn start_initproc() {
    let entry_point = {
        let init_proc = match INIT_DIR.entry("initproc").expect("Cannot find initproc") {
            include_dir::InitDirEntry::File(x) => x,
            _ => panic!("initproc is not a file")
        };
        let elf = Elf::new(init_proc).expect("Wrong elf in init data");
        let ctxp = current_task();
        let mut ctx = ctxp.write();
        ctx.load_elf(&elf);
        unsafe { ctx.prepare_tcb(); }
        ctx.user_entry_point
    };
    //print_tables();

    unsafe { enter_userspace(entry_point) };
}
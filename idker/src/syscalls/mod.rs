use core::convert::TryFrom;

use memoffset::offset_of;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, LStar}
    },
    VirtAddr,
};
use num_enum::TryFromPrimitive;

use crate::println;

mod asm;
mod userspace;

pub use userspace::{enter_userspace, test_prepare_userspace};

#[derive(Clone, Copy, TryFromPrimitive, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallCode {
    EXIT = 0,
    YIELD,// ??
    KPRINT,
}

pub struct ThreadControlData {
    pub user_stack_pointer: u64,
    pub kernel_stack_pointer: u64,
}

#[thread_local]
pub static mut TCD: ThreadControlData = ThreadControlData {
    user_stack_pointer: 0,
    kernel_stack_pointer: 0,
};

pub fn setup_syscalls() {
    LStar::write(VirtAddr::new(on_syscall_raw as u64));
    unsafe { Efer::write(Efer::read() | EferFlags::SYSTEM_CALL_EXTENSIONS) };
}

extern "C" fn on_symcall_1(a: u64, b: u64, c: u64, sel: u64, e: u64, f: u64, d: u64) {
    let regs: &mut asm::AllSavedRegisters = unsafe { &mut *(TCD.user_stack_pointer as *mut _) };

    println!(
        "Syscall 1.{} 2.{} 3.{} 4.{} 5.{} 6.{} 7.{}",
        sel, a, b, c, d, e, f
    );

    let sysnum = match SyscallCode::try_from(sel) {
        Ok(x) => x,
        Err(_) => {
            regs.rax = 1;
            return;
        },
    };

    match sysnum {
        SyscallCode::EXIT => {
            // We need to leave this thread
        },
        SyscallCode::YIELD => todo!(),
        SyscallCode::KPRINT => todo!(),
    }
}

macro_rules! concat_newl {
    ($($x:expr),*) => {
        concat!($($x, "\n"),*)
    };
}

#[naked]
unsafe extern "C" fn on_syscall_raw() {
    //               s.   a.   b.   c.   d.   e.  f.
    // syscall args: rax, rdi, rsi, rdx, r10, r8, r9
    // C args:       rdi, rsi, rdx, rcx, r8, r9,  stack...
    // So we just need to put the selector in rcx (why rcx?
    //   because rcx is not used in the syscall args) and put
    //   r9 in the stack
    asm!(concat_newl!(
        "swapgs",
        "mov fs:[{tcd}@tpoff+{sp_offset}+8], rsp",// save stack pointer
        "mov rsp, fs:[{tcd}@tpoff+{ksp_offset}+8]",// use kernel stack pointer
        asm::save_all_regs!(),
        "mov rcx, rax",// syscall selector
        "push r10",// param d
        "call {c_syscall}",
        "pop r10",// remove param d
        asm::load_all_regs!(),
        "mov rsp, fs:[{tcd}@tpoff+{sp_offset}+8]",// use old stack pointer
        "swapgs",
        "sysretq"),
        tcd = sym TCD,
        sp_offset = const offset_of!(ThreadControlData, user_stack_pointer),
        ksp_offset = const offset_of!(ThreadControlData, kernel_stack_pointer),
        c_syscall = sym on_symcall_1,
        options(noreturn)
    )
}

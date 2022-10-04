use core::convert::TryFrom;

use memoffset::offset_of;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, LStar}
    },
    VirtAddr,
};

use crate::{context::{current_task, switch_to_next_task, task::TaskState}, file::{FileHandleError, PathOpenError}, println};
use super::capability::syscall as cap_call;
use super::context::syscall as proc_call;
use super::file::syscall as file_call;
use syscall::{SyscallCode, SyscallError, SyscallResult};

mod asm;
mod memory;
mod userspace;

pub use userspace::{enter_userspace, start_initproc, check_addr_userspace};


impl From<PathOpenError> for SyscallError {
    fn from(_: PathOpenError) -> SyscallError {
        SyscallError::InvalidPath
    }
}

impl From<FileHandleError> for SyscallError {
    fn from(x: FileHandleError) -> SyscallError {
        match x {
            FileHandleError::NotSeekable => SyscallError::FsNotSeekable,
            FileHandleError::SeekOutOfRange => SyscallError::FsSeekOutOfRange,
        }
    }
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

extern "C" fn on_symcall_1(regs: &mut asm::AllSavedRegisters) {
    //               s.   a.   b.   c.   d.   e.  f.
    // syscall args: rax, rdi, rsi, rdx, r10, r8, r9
    // Warning: syscall overwrites rcx with the return pointer and r11 with the previous rflags value.
    let sel = regs.rax;
    let a = regs.rdi;
    let b = regs.rsi;
    let c = regs.rdx;
    let d = regs.r10;
    let e = regs.r8;
    let f = regs.r9;

    println!(
        "Syscall 1.{} 2.{} 3.{} 4.{} 5.{} 6.{} 7.{}",
        sel, a, b, c, d, e, f
    );

    let sysnum = match SyscallCode::try_from(sel as u64) {
        Ok(x) => x,
        Err(_) => {
            regs.rax = SyscallError::UnknownSyscall as usize;
            return;
        },
    };

    regs.rax = match sysnum {
        SyscallCode::Exit => {
            {
                let lock = current_task();
                let mut task = lock.write();
                task.state = TaskState::Dying;
            }
            switch_to_next_task();
            Ok(())
        }
        SyscallCode::Yield => {
            switch_to_next_task();
            Ok(())
        }

        SyscallCode::FsOpen => {
            file_call::open(a, b, c).map(|fd| regs.rdi = fd.get())
        }
        SyscallCode::FsHandleSeek => {
            file_call::seek(a, b)
        }
        SyscallCode::FsHandleRead => {
            file_call::read(a, b, c).map(|x| regs.rdi = x)
        }
        SyscallCode::FsHandleClose => {
            file_call::close(a)
        }

        SyscallCode::CapabilityClone => {
            cap_call::clone(a).map(|x| regs.rdi = x )
        }
        SyscallCode::CapabilityInspect => {
            cap_call::inspect(a, b).map(|(x, y)| { regs.rdi = x; regs.rsi = y; } )
        }
        SyscallCode::CapabilityRestrict => {
            cap_call::restrict(a, b, c)
        }
        SyscallCode::CapabilityDrop => {
            cap_call::cdrop(a)
        }

        SyscallCode::ProcessMyPid => {
            proc_call::mypid().map(|x| regs.rdi = x.0.get() as usize)
        }
        SyscallCode::ProcessSpawn => {
            proc_call::spawn().map(|x| regs.rdi = x.0.get() as usize)
        }
        SyscallCode::ProcessCapShare | SyscallCode::ProcessCapTransfer => {
            cap_call::process_share_transfer(a, b, sysnum == SyscallCode::ProcessCapTransfer)
        }
        SyscallCode::ProcessExec => {
            proc_call::exec(a, b)
        }

        SyscallCode::MemoryMapVirt => {
            memory::map_virt(a, b, c)
        }
        SyscallCode::MemoryMapPhys => {
            memory::map_phys(a, b, c, d)
        }
        SyscallCode::MemoryUnmap => {
            Ok(())
        }

        _ => Err(SyscallError::UnknownSyscall)
    }.map(|_| 0).unwrap_or_else(|x| x as usize);
}

macro_rules! concat_newl {
    ($($x:expr),*) => {
        concat!($($x, "\n"),*)
    };
}

#[naked]
unsafe extern "C" fn on_syscall_raw() {
    // C args: rdi, rsi, rdx, rcx, r8, r9,  stack...
    core::arch::asm!(concat_newl!(
        "swapgs",
        "mov fs:[{tcd}@tpoff+{sp_offset}+8], rsp",// save stack pointer
        "mov rsp, fs:[{tcd}@tpoff+{ksp_offset}+8]",// use kernel stack pointer
        asm::save_all_regs!(),
        "mov rdi, rsp",
        "call {c_syscall}",
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

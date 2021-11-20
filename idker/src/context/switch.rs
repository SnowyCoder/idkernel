use core::mem;

use super::{UserPageTable, after_task_switch};

use memoffset::offset_of;

// Not-so brief explaination on what's going on here.
// A task has a kernel stack and possibly some user-space thingy, and that's ok
// The basis of a kernel is task-switching, that means putting a task on hold and loading the next one.
// This is ALWAYS explicit, and ALWAYS made from kernel code, that means:
// If you're using firefox to watch cat pictures, maybe the image is too cute and a lot of CPU time will
// be needed to load it properly, after some time the kernel timer will send an interupt to the
// CPU, the interupt will call KERNEL code and the code will say "mmh, yes, maybe we should let other
// programs load cute pictures too" and will call this function.
// This is very important since this code is called with the C ABI so we don't have to save as many
// registers as if we were pausing an user-space thread (or an unknowing kernel thread for that matter).
// I like to think of it as "forced cooperation" :3

#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct ContextRegs {
    pub cr3: usize,
    // Saved registers
    pub rbx: usize,
    pub rbp: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
    pub rsp: usize,// Stack ponter
    pub rflags: usize,// Processor flags
    // fs/gs base?
    //fsbase: usize,
    //gsbase: usize,
}

impl ContextRegs {
    pub fn reload_cr3(&mut self, page_table: &mut UserPageTable) {
        let frame = page_table.get_frame();
        self.cr3 = frame.start_address().as_u64() as usize;
    }

    pub unsafe fn push_stack(&mut self, val: usize) {
        self.rsp -= mem::size_of::<usize>();
        *(self.rsp as *mut usize) = val;
    }
}


// So, what registers should we save?
#[naked]
pub unsafe extern "C" fn switch_task(_from: &ContextRegs, _to: &ContextRegs) {
    // What's the C ABI Again?
    // I'll never remember this so https://aaronbloomfield.github.io/pdr/book/x86-64bit-ccc-chapter.pdf
    // Args:         rdi, rsi, rdx, rcx, r8, r9, stack...
    // Caller saved: r10, r11, and all of the args
    // Callee saved: rbx, rbp, r12, r13, r14, r15
    // So our args are: from -> rdi, to -> rsi
    asm! {
        // Load new cr3 (if necessary)
        "mov rcx, cr3",                     // Get old cr3
        "mov rax, [rsi + {offset_cr3}]",    // Get new cr3
        "cmp rcx, rax",
        "je 2f",
        "mov cr3, rax",
        "2:",   // --------------- Swap regs --------------
        "mov [rdi + {offset_rbx}], rbx",    // Save old rbx
        "mov rbx, [rsi + {offset_rbx}]",    // -Load new rbx

        "mov [rdi + {offset_rbp}], rbp",    // Save old rbp
        "mov rbp, [rsi + {offset_rbp}]",    // -Load new rbp

        "mov [rdi + {offset_r12}], r12",    // Save old r12
        "mov r12, [rsi + {offset_r12}]",    // -Load new r12

        "mov [rdi + {offset_r13}], r13",    // Save old r13
        "mov r13, [rsi + {offset_r13}]",    // -Load new r13

        "mov [rdi + {offset_r14}], r14",    // Save old r14
        "mov r14, [rsi + {offset_r14}]",    // -Load new r14

        "mov [rdi + {offset_r15}], r15",    // Save old r15
        "mov r15, [rsi + {offset_r15}]",    // -Load new r15

        "mov [rdi + {offset_rsp}], rsp",    // Save old stack pointer
        "mov rsp, [rsi + {offset_rsp}]",    // -Load new stack pointer

        // RFLAGS can only be modified by stack:
        // Push RFLAGS to stack
        "pushfq",
        // pop RFLAGS into `from.rflags`
        "pop QWORD PTR [rdi + {offset_rflags}]",
        // push `next.rflags`
        "push QWORD PTR [rsi + {offset_rflags}]",
        // pop into RFLAGS
        "popfq",

        // Call the after switch hook, it will manage the locks
        // and after that do a ret, getting the new rip from the stack and jumping to it
        "jmp {after_task_switch}",

        offset_cr3 = const(offset_of!(ContextRegs, cr3)),
        offset_rbx = const(offset_of!(ContextRegs, rbx)),
        offset_rbp = const(offset_of!(ContextRegs, rbp)),
        offset_r12 = const(offset_of!(ContextRegs, r12)),
        offset_r13 = const(offset_of!(ContextRegs, r13)),
        offset_r14 = const(offset_of!(ContextRegs, r14)),
        offset_r15 = const(offset_of!(ContextRegs, r15)),
        offset_rsp = const(offset_of!(ContextRegs, rsp)),
        offset_rflags = const(offset_of!(ContextRegs, rflags)),
        after_task_switch = sym after_task_switch,

        options(noreturn)
    }
}

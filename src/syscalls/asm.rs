// What's more important? DRY vs KISS?

macro_rules! concat_reverse {
    ([] $($reversed:expr),*) => {
        concat!($($reversed),*)  // base case
    };
    ([$first:expr $(, $rest:expr)*] $($reversed:expr),*) => {
        $crate::syscalls::asm::concat_reverse!([$($rest),*] $first $(, $reversed)*)  // recursion
    };
}

macro_rules! save_specified_regs {
    ([$($x:ident),+]) => {
        $crate::syscalls::asm::concat_reverse!([
            $(concat!("push ", stringify!($x), "\n")),*
        ])
    };
}

macro_rules! load_specified_regs {
    ([$($x:ident),+]) => {
        concat!(
            $(concat!("pop ", stringify!($x), "\n")),*
        )
    };
}

macro_rules! create_specified_reg_struct {
    ($name:ident, $($prex:ident),*, [$($x:ident),+]) => {
        struct $name {
            $($prex: u64,)*
            $($x: u64,)*
        }
    };
}

macro_rules! do_with_all_regs {
    ($x:ident $(, $args:tt)*) => {
        $crate::syscalls::asm::$x!{
            $($args, )*
            [rax, rbx, rcx, rdx, rbp, rsi, rdi, r8, r9, r10, r11, r12, r13, r14, r15]
        }
    }
}

macro_rules! save_all_regs {
    () => {
        concat!(
            $crate::syscalls::asm::do_with_all_regs!(save_specified_regs),
            "pushfq\n",
        )
    };
}

macro_rules! load_all_regs {
    () => {
        concat!(
            "popfq\n",
            $crate::syscalls::asm::do_with_all_regs!(load_specified_regs),
        )
    };
}

do_with_all_regs!(create_specified_reg_struct, AllSavedRegisters, rflags);

pub(crate) use concat_reverse;
pub(crate) use create_specified_reg_struct;
pub(crate) use do_with_all_regs;
pub(crate) use load_all_regs;
pub(crate) use load_specified_regs;
pub(crate) use save_all_regs;
pub(crate) use save_specified_regs;

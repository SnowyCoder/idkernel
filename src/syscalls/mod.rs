use core::{mem::MaybeUninit, ptr};

use alloc::boxed::Box;
use memoffset::offset_of;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, LStar},
        rflags::RFlags,
    },
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, Translate,
    },
    VirtAddr,
};

use crate::{allocator::get_frame_allocator, arch::paging::get_page_table, println};

mod asm;

struct ThreadControlData {
    user_stack_pointer: u64,
    kernel_stack_pointer: u64,
}

#[thread_local]
static mut TCD: ThreadControlData = ThreadControlData {
    user_stack_pointer: 0,
    kernel_stack_pointer: 0,
};

pub fn setup_syscalls() {
    LStar::write(VirtAddr::new(on_syscall_raw as u64));
    unsafe { Efer::write(Efer::read() | EferFlags::SYSTEM_CALL_EXTENSIONS) };
}

extern "C" fn on_symcall_1(a: usize, b: usize, c: usize, sel: usize, e: usize, f: usize, d: usize) {
    println!(
        "Syscall 1.{} 2.{} 3.{} 4.{} 5.{} 6.{} 7.{}",
        sel, a, b, c, d, e, f
    );
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
    // to reduce the arguments moved we only
    asm!(concat_newl!(
        asm::save_all_regs!(),
        "swapgs",
        //"push rcx",// save IP
        //"push r11",// save rflags
        "mov fs:[{tcd}@tpoff+{sp_offset}+8], rsp",// save stack pointer
        "mov rsp, fs:[{tcd}@tpoff+{ksp_offset}+8]",// use kernel stack pointer
        "mov rcx, rax",// syscall selector
        "push r10",// param d
        "call {c_syscall}",
        "mov rsp, fs:[{tcd}@tpoff+{sp_offset}+8]",// save stack pointer
        "swapgs",
        asm::load_all_regs!(),
        "sysretq"),
        tcd = sym TCD,
        sp_offset = const offset_of!(ThreadControlData, user_stack_pointer),
        ksp_offset = const offset_of!(ThreadControlData, kernel_stack_pointer),
        c_syscall = sym on_symcall_1,
        options(noreturn)
    )
}
//trace_macros!(false);

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

pub unsafe fn test_prepare_userspace() -> VirtAddr {
    const USERSPACE_STACK_ADDR: u64 = 0x4000_0000;
    const USERSPACE_CODE_ADDR: u64 = 0x5000_0000;

    const STACK_SIZE: usize = 4096;
    type UserStack = [u8; STACK_SIZE];
    let stack: &mut MaybeUninit<UserStack> = Box::leak(Box::new_uninit());
    let mut page_table = get_page_table();
    let mut frame_allocator = get_frame_allocator();
    let real_stack = page_table
        .translate_addr(VirtAddr::new(stack as *const _ as u64))
        .unwrap();
    // Map stack
    page_table
        .map_to(
            Page::containing_address(VirtAddr::new(USERSPACE_STACK_ADDR)),
            PhysFrame::<Size4KiB>::containing_address(real_stack),
            PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE
                | PageTableFlags::USER_ACCESSIBLE,
            &mut *frame_allocator,
        )
        .unwrap()
        .flush();

    // Map func
    let func_phys1 = frame_allocator.allocate_frame().unwrap();
    let src_func_page = Page::containing_address(VirtAddr::new(USERSPACE_CODE_ADDR));
    page_table
        .map_to(
            src_func_page,
            func_phys1,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            &mut *frame_allocator,
        )
        .unwrap()
        .flush();
    let func_phys2 = frame_allocator.allocate_frame().unwrap();
    page_table
        .map_to(
            src_func_page + 1,
            func_phys2,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            &mut *frame_allocator,
        )
        .unwrap()
        .flush();

    let func_addr = VirtAddr::new(test_userspace as *const u8 as u64);
    ptr::copy_nonoverlapping::<u8>(func_addr.as_ptr(), USERSPACE_CODE_ADDR as *mut u8, 4096 * 2);

    TCD.user_stack_pointer = USERSPACE_STACK_ADDR + STACK_SIZE as u64 - 128;

    VirtAddr::new(USERSPACE_CODE_ADDR)
}

#[no_mangle]
#[naked]
unsafe extern "C" fn test_userspace() {
    asm! {
        "push rax",
        "mov rax, 1",
        "mov rdi, 2",
        "mov rsi, 3",
        "mov rdx, 4",
        "mov r10, 5",
        "mov r8, 6",
        "mov r9, 7",
        "syscall",
        "2: jmp 2b",
        options(noreturn)
    }
}

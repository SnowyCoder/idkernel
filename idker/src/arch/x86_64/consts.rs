use bootloader_api::{info::Optional, BootInfo};

pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;
// Actually the first 0x0010_0000 (1MiB) addresses are not mapped! (zero page trap? oh well)
const KIB: u64 = 1024; // 2**10
const MIB: u64 = KIB * KIB; // 2**20
const GIB: u64 = MIB * KIB; // 2**30
const TIB: u64 = GIB * KIB; // 2**40
const SPACING_16_TIB: u64 = 16 * TIB;

// Bootloader info
pub const KERNEL_BOOTDATA_START: u64 = KERNEL_BASE + SPACING_16_TIB; // 0xFFFF_9000_0000_0000
pub const KERNEL_BOOTDATA_BOOTINFO: u64 = KERNEL_BOOTDATA_START;
pub const KERNEL_BOOTDATA_FRAMEBUFFER: u64 = KERNEL_BOOTDATA_BOOTINFO + GIB; // 0xFFFF_9000_4000_0000
pub const KERNEL_INITIAL_STACK: u64 = KERNEL_BOOTDATA_FRAMEBUFFER + GIB; // 0xFFFF_9000_8000_0000

pub const KERNEL_PHYSICAL_MEMORY_START: u64 = KERNEL_BOOTDATA_START + SPACING_16_TIB; // 0xFFFF_A000_0000_0000

// Kernel thread data: Stack | TLS (remember that the stack grows down so the trap page
//   must be before the TLS)
// Note: bootloader needs cpu 0 stack address, so the first address will be used
pub const KERNEL_THREAD_DATA_START: u64 = KERNEL_PHYSICAL_MEMORY_START + SPACING_16_TIB; // 0xFFFF_B000_0000_0000
pub const KERNEL_THREAD_STORAGE_SIZE: u64 = 64 * KIB;

pub const KERNEL_HEAP_START: u64 = KERNEL_THREAD_DATA_START + SPACING_16_TIB; // 0xFFFF_C000_0000_0000

pub fn check_boot_info(binfo: &BootInfo) {
    assert!(
        binfo.framebuffer.as_ref().unwrap().buffer().as_ptr() as u64 == KERNEL_BOOTDATA_FRAMEBUFFER
    );
    assert!(binfo.physical_memory_offset == Optional::Some(KERNEL_PHYSICAL_MEMORY_START));
    // TODO: kernel check?
}

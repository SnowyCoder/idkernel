use lazy_static::lazy_static;
use x86_64::{
    registers::model_specific::Star,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

// GDT = Global DescriptorTable
// It contains entries about the memory segments

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

#[derive(Debug)]
struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
}

#[thread_local]
static mut TSS: TaskStateSegment = TaskStateSegment::new();

const DOUBLE_FAULT_STACK_SIZE: usize = 2 * 4096; // 16KiB

#[thread_local]
static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0u8; DOUBLE_FAULT_STACK_SIZE];

const R3_TO_R0_INT_SIZE: usize = 2 * 4096; // 16KiB

#[thread_local]
static mut R3_TO_R0_INT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0u8; R3_TO_R0_INT_SIZE];

#[thread_local]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

unsafe fn init_tss() {
    TSS.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let stack_start = VirtAddr::from_ptr(&DOUBLE_FAULT_STACK);
        let stack_end = stack_start + DOUBLE_FAULT_STACK_SIZE;
        stack_end
    };
    TSS.privilege_stack_table[0] = {
        let stack_start = VirtAddr::from_ptr(&R3_TO_R0_INT_STACK);
        let stack_end = stack_start + R3_TO_R0_INT_STACK.len();
        stack_end
    };
}

unsafe fn init_gdt() -> Selectors {
    init_tss();

    let code_selector = GDT.add_entry(Descriptor::kernel_code_segment());
    let data_selector = GDT.add_entry(Descriptor::kernel_data_segment());
    let tss_selector = GDT.add_entry(Descriptor::tss_segment(&TSS));
    let user_data_selector = GDT.add_entry(Descriptor::user_data_segment());
    let user_code_selector = GDT.add_entry(Descriptor::user_code_segment());
    Selectors {
        code_selector,
        data_selector,
        tss_selector,
        user_code_selector,
        user_data_selector,
    }
}

lazy_static! {
    static ref GTSS: TaskStateSegment = TaskStateSegment::new();
}

lazy_static! {
    static ref GGDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&GTSS));
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                tss_selector,
                // False values, this is only used before proper thread_local
                user_code_selector: code_selector,
                user_data_selector: data_selector,
            },
        )
    };
}

pub fn init_prepaging() {
    use x86_64::instructions::{
        segmentation::{Segment, CS, DS, ES, SS},
        tables::load_tss,
    };

    unsafe {
        GGDT.0.load();

        CS::set_reg(GGDT.1.code_selector);
        SS::set_reg(GGDT.1.data_selector);
        DS::set_reg(SegmentSelector(0));
        ES::set_reg(SegmentSelector(0));

        load_tss(GGDT.1.tss_selector);
    }
}

pub fn init() {
    use x86_64::instructions::{
        segmentation::{Segment, CS, DS, ES, SS},
        tables::load_tss,
    };

    let selectors = unsafe {
        let selectors = init_gdt();

        GDT.load();

        CS::set_reg(selectors.code_selector);
        SS::set_reg(selectors.data_selector);
        DS::set_reg(SegmentSelector(0));
        ES::set_reg(SegmentSelector(0));

        load_tss(selectors.tss_selector);
        selectors
    };

    Star::write(
        selectors.user_code_selector,
        selectors.user_data_selector,
        selectors.code_selector,
        selectors.data_selector,
    )
    .unwrap();
}

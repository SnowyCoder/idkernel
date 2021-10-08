use lazy_static::lazy_static;
use x86_64::{
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

// GDT = Global DescriptorTable
// It contains entries about the memory segments

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

#[thread_local]
static mut TSS: TaskStateSegment = TaskStateSegment::new();

const DOUBLE_FAULT_STACK_SIZE: usize = 4096; // 4Kb

#[thread_local]
static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0u8; DOUBLE_FAULT_STACK_SIZE];

#[thread_local]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

unsafe fn init_tss() {
    TSS.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let stack_start = VirtAddr::from_ptr(&DOUBLE_FAULT_STACK);
        let stack_end = stack_start + DOUBLE_FAULT_STACK_SIZE;
        stack_end
    };
}

unsafe fn init_gdt() -> Selectors {
    init_tss();

    let code_selector = GDT.add_entry(Descriptor::kernel_code_segment());
    let data_selector = GDT.add_entry(Descriptor::kernel_data_segment());
    let tss_selector = GDT.add_entry(Descriptor::tss_segment(&TSS));
    Selectors {
        code_selector,
        data_selector,
        tss_selector,
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

    unsafe {
        let selectors = init_gdt();

        GDT.load();

        CS::set_reg(selectors.code_selector);
        SS::set_reg(selectors.data_selector);
        DS::set_reg(SegmentSelector(0));
        ES::set_reg(SegmentSelector(0));

        load_tss(selectors.tss_selector);
    }
}

use crate::println;
use x86_64::VirtAddr;
use raw_cpuid::CpuId;
use x86_64::registers::model_specific::Msr;

const MSR_IA32_APIC_BASE: Msr = Msr::new(0x1B);
// x2APIC ID register (READ ONLY)
const MSR_IA32_X2APIC_APICID: Msr = Msr::new(0x802);
// x2APIC VERSION register (READ ONLY)
const MSR_IA32_X2APIC_VERSION: Msr = Msr::new(0x803);
// x2APIC Interrupt Command Register
const MSR_IA32_X2APIC_ICR: Msr = Msr::new(0x830);
// x2APIC Spurious Interrupt Vector Register
const MSR_IA32_X2APIC_SIVR: Msr = Msr::new(0x80f);

const APIC_BASE: u32 = 0xFEE00000;

const APICREG_SUPRIOUS: u32 = 0xF0;
// Used toi tell the LAPIC the End Of Interrupt signal (write 0 to the register)
const APICREG_EOI: u32 = 0xB0;

// 0:7  -> interrupt vector to be fired
// 8:11 -> type of interrupt (000 normal, 100 Non Maskable)
// 12   -> interrupt status (0 = idle, 1 = interrupt pending)
// 13   -> polarity (0: active low, 1: active high)
// 14   -> ACK signal for level triggered interrupts
// 15   -> trigger mode (0 edge triggered, 1 level triggered)
// 16   -> mask bit (1 = mask the IRQ, NMI still fires)
const APICREG_LINT0: u32 = 0x350;
const APICREG_LINT1: u32 = 0x360;
// 0:7 -> interrupt vector to be fired
// 12  -> interrupt status (0 = idle, 1 = interrupt pending)
// 16  -> mask bit (1 = mask the IRQ)
const APICREG_TIMER: u32 = 0x320;

pub static mut LOCAL_APIC: LocalApic = LocalApic {
    base_address: VirtAddr::zero(),
    is_ver2: false
};

pub struct LocalApic {
    pub base_address: VirtAddr,
    pub is_ver2: bool,
}

#[derive(Debug)]
pub enum InitError {
    ApicNotSupported,
}

impl LocalApic {
    pub unsafe fn init(&mut self, base_address: VirtAddr) -> Result<(), InitError> {
        let cpuid = CpuId::new();
        let features = cpuid.get_feature_info().unwrap();
        if !features.has_apic() {
            return Err(InitError::ApicNotSupported);
        }

        println!("APIC BASE: {:?}", base_address);
        self.base_address = base_address;
        self.is_ver2 = features.has_x2apic();

        self.init_ap();
        println!("APIC ID: {}", self.id());
        println!("APIC VERSION: {}", self.version());
        Ok(())
    }

    unsafe fn init_ap(&mut self) {
        if self.is_ver2 {
            // Enable the Local APIC
            MSR_IA32_APIC_BASE.write(MSR_IA32_APIC_BASE.read() | 1 << 10);
            // Set the Spurious Interrupt Vector Register bit 8 to start receiving interrupts
            MSR_IA32_X2APIC_SIVR.write(0x100);
        } else {
            // Set the Spurious Interrupt Vector Register bit 8 to start receiving interrupts
            self.write(APICREG_SUPRIOUS, 0x100);
        }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        (self.base_address + reg as usize).as_ptr::<u32>().read_volatile()
    }

    unsafe fn write(&self, reg: u32, val: u32) {
        (self.base_address + reg as usize).as_mut_ptr::<u32>().write_volatile(val);
    }

    pub fn id(&self) -> u32 {
        if self.is_ver2 {
            unsafe { MSR_IA32_X2APIC_APICID.read() as u32 }
        } else {
            unsafe { self.read(0x20) }
        }
    }

    pub fn version(&self) -> u32 {
        if self.is_ver2 {
            unsafe { MSR_IA32_X2APIC_VERSION.read() as u32 }
        } else {
            unsafe { self.read(0x30) }
        }
    }

    /// Get the Interrupt Command Register
    pub fn icr(&self) -> u64 {
        if self.is_ver2 {
            unsafe { MSR_IA32_X2APIC_ICR.read() }
        } else {
            unsafe {
                (self.read(0x310) as u64) << 32 | self.read(0x300) as u64
            }
        }
    }

    pub fn set_icr(&mut self, value: u64) {
        if self.is_ver2 {
            unsafe { MSR_IA32_X2APIC_ICR.write(value) }
        } else {
            const DELIVERY_MASK: u32 = 1 << 12;
            unsafe {
                while self.read(0x300) & DELIVERY_MASK != 0 {}
                self.write(0x310, (value >> 32) as u32);
                self.write(0x300, value as u32);
                while self.read(0x300) & DELIVERY_MASK != 0 {}
            }
        }
    }
}

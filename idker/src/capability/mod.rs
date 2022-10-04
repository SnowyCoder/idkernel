use bitflags::bitflags;
use x86_64::PhysAddr;

pub mod syscall;

bitflags! {
    pub struct CapabilityPerms: u32 {
        const DUPLICATE = 0x1;
        // If a capability can be shared a single capability can be used by multiple processes
        // (ex. RamVirtMap, cannot be duplicated and each time it's used is expended)
        const SHAREABLE = 0x2;
        const TRANSFER  = 0x4;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    pub perms: CapabilityPerms,
    pub ctype: CapabilityType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityType {
    // Allows to map physical ranges (and specifies which ranges you can map) to virtual addresses
    MapPhysical(PhysAddr, PhysAddr),
    // Allows to create RAM-backed virtual mappings. (with optional RAM limits)
    MapVirtualToRam,// todo: memory limits
    // Allows to spawn new processes
    ProcessSpawn,
    // Allows creation of new channels
    ChannelCreate,
}

impl CapabilityType {
    pub fn cap_id(&self) -> usize {
        match &self {
            CapabilityType::MapPhysical(_, _) => 1,
            CapabilityType::MapVirtualToRam => 2,
            CapabilityType::ProcessSpawn => 3,
            CapabilityType::ChannelCreate => 4,
        }
    }
}

use core::num::NonZeroU64;

use alloc::vec::Vec;
use x86_64::PhysAddr;

use crate::{context::{TaskId, current_task, current_task_id, tasks}};
use syscall::{SyscallError, SyscallResult};

use super::{Capability, CapabilityPerms, CapabilityType};


pub type CapabilityHandle = usize;

#[derive(Clone, Default, Debug)]
pub struct TaskCapabilityStorage {
    next_handle: CapabilityHandle,
    // This should be a map but BTreeMap does not support fallible allocations and HashMap isn't in alloc...
    // It should be fine since a program won't have more than 10-20 capabilities (i hope)
    pub handles: Vec<(CapabilityHandle, Capability)>,
}

impl TaskCapabilityStorage {
    pub fn insert(&mut self, cap: Capability) -> SyscallResult<CapabilityHandle> {
        let id = self.next_handle;
        self.next_handle += 1;
        self.handles.try_reserve(1)?;
        self.handles.push((id, cap));
        Ok(id)
    }

    pub fn get(&self, handle: CapabilityHandle) -> Option<&Capability> {
        self.handles.iter()
                .find(|x| x.0 == handle)
                .map(|x| &x.1)
    }

    pub fn get_mut(&mut self, handle: CapabilityHandle) -> Option<&mut Capability> {
        self.handles.iter_mut()
                .find(|x| x.0 == handle)
                .map(|x| &mut x.1)
    }

    pub fn delete(&mut self, handle: CapabilityHandle) -> SyscallResult<Capability> {
        let index = self.handles.iter()
                .position(|x| x.0 == handle)
                .ok_or(SyscallError::WrongCapability)?;
        Ok(self.handles.remove(index).1)
    }
}

pub fn clone(cap_handle: CapabilityHandle) -> SyscallResult<CapabilityHandle> {
    let task_lock = current_task();
    let mut task = task_lock.write();
    let caps = &mut task.capabilities;
    let cap = caps.get(cap_handle)
            .ok_or(SyscallError::WrongCapability)?;

    if !cap.perms.contains(CapabilityPerms::DUPLICATE) {
        return Err(SyscallError::WrongCapabilityPerms)
    }

    let cap = cap.clone();
    caps.insert(cap)
}

pub fn inspect(what: usize, index: usize) -> SyscallResult<(usize, usize)> {
    let task_lock = current_task();
    let task = task_lock.read();
    match what {
        0 => Ok((task.capabilities.handles.len(), 0)),
        1 => {
            task.capabilities.handles.get(index)
                    .map(|x| (x.0, x.1.ctype.cap_id()))
                    .ok_or(SyscallError::WrongParameters)
        }
        2 => {
            task.capabilities.get(index)
                    .map(|x| (x.perms.bits() as usize, x.ctype.cap_id()))
                    .ok_or(SyscallError::WrongCapability)
        }
        _ => Err(SyscallError::WrongParameters),
    }
}

pub fn restrict(handle: CapabilityHandle, a: usize, b: usize) -> SyscallResult<()> {
    let task_lock = current_task();
    let mut task = task_lock.write();
    let cap = task.capabilities.get_mut(handle).ok_or(SyscallError::WrongCapability)?;
    match &mut cap.ctype {
        CapabilityType::MapPhysical(from, to) => {
            if a > b { return Err(SyscallError::WrongParameters) }
            let nfrom = PhysAddr::try_new(a as u64).map_err(|_| SyscallError::WrongParameters)?;
            let nto = PhysAddr::try_new(b as u64).map_err(|_| SyscallError::WrongParameters)?;
            if nfrom < *from || nto > *to {
                return Err(SyscallError::WrongParameters);
            }
            *from = nfrom;
            *to = nto;
            Ok(())
        },
        CapabilityType::MapVirtualToRam => Err(SyscallError::NotImplemented),
        _ => Err(SyscallError::WrongParameters),
    }
}

pub fn cdrop(handle: CapabilityHandle) -> SyscallResult<()> {
    let task_lock = current_task();
    let mut task = task_lock.write();
    task.capabilities.delete(handle)
            .map(|_| ())// Drop capability
}

pub fn process_share_transfer(proc: usize, cap_handle: CapabilityHandle, transfer: bool) -> SyscallResult<()> {
    let proc = TaskId(NonZeroU64::new(proc as u64).ok_or(SyscallError::WrongParameters)?);

    if proc == current_task_id() {
        return Err(SyscallError::WrongProcess);
    }

    let cap = if transfer {
        let current_lock = current_task();
        let mut current = current_lock.write();

        current.capabilities.delete(cap_handle)?
    } else {
        let current_lock = current_task();
        let current = current_lock.read();
        let cap = current.capabilities.get(cap_handle)
                .ok_or(SyscallError::WrongCapability)?;

        if !cap.perms.contains(CapabilityPerms::SHAREABLE) {
            return Err(SyscallError::WrongCapabilityPerms);
        }
        cap.clone()
    };

    let target_lock = {
        let tasks = tasks();
        tasks.get(proc)
                .ok_or(SyscallError::WrongProcess)?
                .clone()
    };

    let mut target = target_lock.write();

    if target.parent != Some(current_task_id()) {
        return Err(SyscallError::WrongProcess);
    }

    let _new_handle = target.capabilities.insert(cap)?;

    Ok(())
}
use core::num::NonZeroU64;

use x86_64::VirtAddr;

use crate::{capability::CapabilityType, file::read_file_to_memory, println, syscalls::{enter_userspace}};
use syscall::{SyscallError, SyscallResult};

use super::{TaskContext, TaskId, current_task, current_task_id, elf::Elf, tasks, tasks_mut};


pub fn mypid() -> SyscallResult<TaskId> {
    Ok(current_task_id())
}

pub fn spawn() -> SyscallResult<TaskId> {
    let task_lock = current_task();
    let task = task_lock.write();
    // Check capability
    task.capabilities.handles.iter()
            .find(|x| x.1.ctype == CapabilityType::ProcessSpawn)
            .ok_or(SyscallError::WrongCapability)?;

    let child = TaskContext::create(current_task_id(), jmp_userspace);
    let child_id = child.id;
    tasks_mut().add(child);

    Ok(child_id)
}

pub fn exec(proc_id: usize, fd: usize) -> SyscallResult<()> {
    let proc_id = TaskId(NonZeroU64::new(proc_id as u64).ok_or(SyscallError::WrongParameters)?);
    let proc_guard = tasks().get(proc_id)
            .ok_or(SyscallError::WrongProcess)?
            .clone();
    let mut proc = proc_guard.write();
    if proc.parent != Some(current_task_id()) {
        return Err(SyscallError::WrongProcess);
    }
    if proc.user_entry_point != VirtAddr::zero() {
        return Err(SyscallError::WrongProcess);
    }
    let curr_guard = current_task();
    let mut curr = curr_guard.write();

    let file_handle =  &mut curr.files.handles.iter_mut()
            .find(|x| x.0.get() == fd)
            .ok_or(SyscallError::WrongDescriptor)?.1;

    // Limit: 1 GiB
    let file = read_file_to_memory(file_handle.as_mut(), 1024 * 1024 * 1024)?;
    let elf = Elf::new(&file).map_err(|_| SyscallError::FsNotExecutable)?;
    proc.load_elf(&elf);
    tasks_mut().queue_for_execution(proc.id);
    // then start program

    Ok(())
}

extern fn jmp_userspace() {
    let entry_point = {
        let ctxp = current_task();
        let ctx = ctxp.read();
        ctx.user_entry_point
    };
    //print_tables();
    if entry_point == VirtAddr::zero() {
        println!("Tried to jump into an uninitialized task");
    } else {
        unsafe { enter_userspace(entry_point) };
    }
}
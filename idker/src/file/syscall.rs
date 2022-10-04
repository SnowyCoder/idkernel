use core::{num::NonZeroUsize, slice};

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use spin::RwLock;
use syscall::FsOpenMode;

use crate::{context::{TaskId, current_task}, syscalls::{check_addr_userspace}};
use ::syscall::{SyscallError, SyscallResult};

use super::{procfs::{ProcRootFs, ProcRootPathHandle}, system::{FileHandle, open_path_full}};

pub type FileDescriptor = NonZeroUsize;

pub struct TaskFileStorage {
    pub root: Arc<RwLock<ProcRootFs>>,
    pub handles: Vec<(FileDescriptor, Box<dyn FileHandle>)>,
    next_descriptor: FileDescriptor,
}

impl TaskFileStorage {
    pub fn allocate_descriptor(&mut self) -> FileDescriptor {
        let x = self.next_descriptor;
        self.next_descriptor = NonZeroUsize::new(x.get() + 1).unwrap();
        x
    }
}

impl TaskFileStorage {
    pub fn new(task: TaskId) -> Self {
        TaskFileStorage {
            root: Arc::new(RwLock::new(ProcRootFs::new(task))),
            handles: Vec::new(),
            next_descriptor: NonZeroUsize::new(1).unwrap(),
        }
    }
}

pub fn open(path_ptr: usize, path_len: usize, mode: usize) -> SyscallResult<FileDescriptor> {
    check_addr_userspace(path_ptr)?;
    check_addr_userspace(path_ptr + path_len)?;
    let path = unsafe { slice::from_raw_parts(path_ptr as *const u8, path_len) };
    let path = core::str::from_utf8(path)?;
    let mode = FsOpenMode::from_bits_truncate(mode as u8);

    if mode.contains(FsOpenMode::WRITE) {
        todo!();
    }

    let task_lock = current_task();
    let mut task = task_lock.write();
    let root_handle = ProcRootPathHandle(task.files.root.clone());
    let path_handle = open_path_full(Arc::new(root_handle), path)?;
    let file_handle = path_handle.read();
    let descriptor = task.files.allocate_descriptor();
    task.files.handles.push((descriptor, file_handle));
    Ok(descriptor)
}

pub fn seek(fd: usize, index: usize) -> SyscallResult<()> {
    let task_lock = current_task();
    let mut task = task_lock.write();
    let handle = &mut task.files.handles.iter_mut()
            .find(|x| x.0.get() == fd)
            .ok_or(SyscallError::WrongDescriptor)?.1;

    handle.seek(index)?;
    Ok(())
}

pub fn read(fd: usize, at: usize, length: usize) -> SyscallResult<usize> {
    // TODO: is it safe to write?
    check_addr_userspace(at)?;
    check_addr_userspace(at + length)?;
    let slice = unsafe { core::slice::from_raw_parts_mut(at as *mut u8, length) };

    let task_lock = current_task();
    let mut task = task_lock.write();
    let handle = &mut task.files.handles.iter_mut()
            .find(|x| x.0.get() == fd)
            .ok_or(SyscallError::WrongDescriptor)?.1;

    Ok(handle.read(slice))
}

pub fn close(fd: usize) -> SyscallResult<()> {
    let task_lock = current_task();
    let mut task = task_lock.write();
    let index = task.files.handles.iter()
            .position(|x| x.0.get() == fd)
            .ok_or(SyscallError::WrongDescriptor)?;

    task.files.handles.remove(index);
    Ok(())
}
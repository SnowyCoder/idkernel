use core::num::NonZeroU64;

use crate::{raw::*, SyscallResult, FsOpenMode, SyscallError};


pub fn exit() -> ! {
    unsafe { raw_exit() }.unwrap();
    loop {}
}

pub fn yield_() -> () {
    unsafe { raw_yield() }.unwrap();
}


#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct File(NonZeroU64);

impl File {
    pub fn new(path: &str, mode: FsOpenMode) -> SyscallResult<File> {
        let res = unsafe { raw_fs_open(path.as_ptr() as u64, path.len() as u64, mode.bits() as u64) }?;
        let res = NonZeroU64::new(res).ok_or(SyscallError::UnknownError)?;
        Ok(File(res))
    }

    pub fn seek(&self, at: u64) -> SyscallResult<()> {
        unsafe { raw_fs_handle_seek(self.0.get(), at) }
    }

    pub fn read<'a>(&self, buf: &'a mut [u8]) -> SyscallResult<&'a mut [u8]> {
        let len = unsafe { raw_fs_handle_read(self.0.get(), buf.as_ptr() as u64, buf.len() as u64) }?;
        Ok(&mut buf[..len as usize])
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { raw_fs_handle_close(self.0.get()) }.expect("Cannot close FileDescriptor")
    }
}

pub struct Process(NonZeroU64);

impl Process {
    pub fn my_pid() -> SyscallResult<u64> {
        unsafe { raw_process_my_pid() }
    }

    pub fn spawn() -> SyscallResult<Self> {
        let pid = unsafe { raw_process_spawn() }?;
        let pid = NonZeroU64::new(pid).ok_or(SyscallError::UnknownError)?;
        Ok(Process(pid))
    }

    pub fn capability_share(&self, cap_id: u64) -> SyscallResult<()> {
        unsafe { raw_process_cap_share(self.0.get(), cap_id) }
    }

    pub fn capability_transfer(&self, cap_id: u64) -> SyscallResult<()> {
        unsafe { raw_process_cap_transfer(self.0.get(), cap_id) }
    }

    pub fn exec(path: &str) -> SyscallResult<()> {
        unsafe { raw_process_exec(path.as_ptr() as u64, path.len() as u64) }
    }
}
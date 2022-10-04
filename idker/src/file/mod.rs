mod system;
mod initfs;
mod procfs;
pub mod syscall;

use core::cmp::min;

use alloc::vec::Vec;
pub use system::{PathHandle, PathNavigator, PathType, PathOpenError, FileHandle, FileHandleError};
pub use initfs::{InitFsFolderHandle, InitFsFileHandle};

use ::syscall::{SyscallError, SyscallResult};

pub fn read_file_to_memory(fh: &mut dyn FileHandle, limit: usize) -> SyscallResult<Vec<u8>> {
    const PAGE: usize = 4096;
    let mut buffer = Vec::new();
    loop {
        let read_n = min(PAGE, limit - buffer.len());
        if read_n == 0 {
            return Err(SyscallError::NoMemory);
        }
        buffer.try_reserve(read_n)?;
        let old_len = buffer.len();
        buffer.resize(old_len, 0);
        let slice = &mut buffer[old_len..];
        let read = fh.read(slice);
        if read != read_n {
            buffer.truncate(old_len + read);
        }
        if read == 0 {
            break;
        }
    }

    Ok(buffer)
}

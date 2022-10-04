
use core::{str::Utf8Error};

use num_enum::TryFromPrimitive;
use bitflags::bitflags;

#[derive(Clone, Copy, TryFromPrimitive, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallCode {
    Exit = 0,
    Yield,

    CapabilityClone = 0x100,// Clones capability
    // Inspects a capability
    // It has 3 modalities: 0 -> length, 1 -> read index, 2 -> read capability
    // In 0 it returns the capability count
    // With 1 or 2 it returns (handle, capability type)
    // Difference between 1 and 2 is that with 1 capabilities are indexed linearly (0..length) while with 1 they are indexed by their handle.
    CapabilityInspect,
    CapabilityRestrict,// Restricts a capability (the parameters depend on which capability we're restricting)
    CapabilityDrop,// Drops a capability

    FsOpen = 0x200,// opens a path, arguments: path: &str (as a &[u8]), mode: OpenMode -> Handle (usize)
    FsHandleSeek,// args: handle, index
    FsHandleRead,// args: handle, &mut [u8]
    FsHandleWrite,// args: handle, &mut [u8]
    FsHandleClose,

    // TODO: implement conversations
    ConversationCreateP2p = 0x300,// Creates a p2p conversation (requires capability)
    ConversationCreatePublic,// Creates a non-p2p conversation (requires capability)
    ConversationTalk,// Sends a binary message to the conversation
    ConversationCapShare,// Shares a capability to the conversation
    ConversationCapTransfer,// Transfers a capability to a p2p conversation

    ProcessMyPid = 0x400,// Returns current process pid
    ProcessSpawn,// Spawns a new empty process (requires capability)
    ProcessCapShare,// Share a capability with a child process (needs to be empty)
    ProcessCapTransfer,// Transfer a capability to a child process (needs to be empty)
    ProcessExec,// Starts a program in an empty process, maintaining its capabilities

    MemoryMapVirt = 0x500,// Maps virtual memory to RAM (requires capability) params: vfrom-vlen, perms
    //MemoryMapFile?
    MemoryMapPhys,// Maps virtual memoty to physical (requires capability) params: vrom-vlen tfrom, perms
    MemoryEditPerms,// Change permissions of page ranges
    MemoryUnmap,// Unmaps previously mapped memory vfrom-vlen
}


#[derive(Clone, Copy, TryFromPrimitive, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallError {
    UnknownSyscall = 1,
    NotImplemented,// Syscall is correct but is left yet to implement
    WrongParameters,
    NoMemory,
    StringNotUtf8,
    InvalidPath,
    WrongDescriptor,
    FsNotSeekable,
    FsSeekOutOfRange,
    FsNotExecutable,
    WrongCapability,
    WrongCapabilityPerms,
    WrongProcess,
    MemoryAlreadyMapped,
    UnknownError = u64::MAX,
}

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
impl From<alloc::collections::TryReserveError> for SyscallError {
    fn from(_: alloc::collections::TryReserveError) -> SyscallError {
        SyscallError::NoMemory
    }
}

impl From<Utf8Error> for SyscallError {
    fn from(_: Utf8Error) -> SyscallError {
        SyscallError::StringNotUtf8
    }
}


pub type SyscallResult<T> = Result<T, SyscallError>;

bitflags! {
    pub struct FsOpenMode: u8 {
        const READ = 0x1;
        const WRITE = 0x2;
        //const APPEND = 0x4; ?
    }
}
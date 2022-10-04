use syscall_macros::create_syscall;
use crate::SyscallCode;
use crate::SyscallError;
use crate::SyscallResult;


create_syscall!(raw_exit, Exit, 0, 0);
create_syscall!(raw_yield, Yield, 0, 0);

// FileSystem
create_syscall!(raw_fs_open, FsOpen, 3, 1);
create_syscall!(raw_fs_handle_seek, FsHandleSeek, 2, 0);
create_syscall!(raw_fs_handle_read, FsHandleRead, 3, 1);
//create_syscall!(raw_fs_handle_write, FsHandleWrite, 3, 1); TODO
create_syscall!(raw_fs_handle_close, FsHandleClose, 1, 0);

// Capability
create_syscall!(raw_capability_clone, CapabilityClone, 1, 1);
create_syscall!(raw_capability_inspect, CapabilityInspect, 2, 2);
create_syscall!(raw_capability_restrict, CapabilityRestrict, 3, 0);
create_syscall!(raw_capability_drop, CapabilityDrop, 1, 0);

// Process
create_syscall!(raw_process_my_pid, ProcessMyPid, 0, 1);
create_syscall!(raw_process_spawn, ProcessSpawn, 0, 1);
create_syscall!(raw_process_cap_share, ProcessCapShare, 2, 0);
create_syscall!(raw_process_cap_transfer, ProcessCapTransfer, 2, 0);
create_syscall!(raw_process_exec, ProcessExec, 2, 0);

// Virt Mem
create_syscall!(raw_memory_map_virt, MemoryMapVirt, 3, 0);
create_syscall!(raw_memory_map_phys, MemoryMapPhys, 4, 0);
//create_syscall!(raw_memory_unmap, MemoryUnmap, 4, 0); TODO

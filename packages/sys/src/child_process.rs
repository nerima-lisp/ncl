use core::ffi::c_int;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

#[repr(C)]
struct ResourceLimit {
    current: u64,
    maximum: u64,
}

const RLIMIT_CPU: c_int = 0;

// SAFETY: this declaration uses the POSIX C ABI and is called only with a valid resource limit.
unsafe extern "C" {
    fn setrlimit(resource: c_int, limit: *const ResourceLimit) -> c_int;
}

/// Limit a child's CPU time before it executes its program.
///
/// # Errors
///
/// Returns an error from [`Command::spawn`] if the child cannot apply the limit.
pub fn set_child_cpu_time_limit(command: &mut Command, seconds: u64) -> io::Result<()> {
    let set_limit = move || {
        let limit = ResourceLimit {
            current: seconds,
            maximum: seconds,
        };
        // SAFETY: `limit` has the POSIX `rlimit` layout and remains live for this call.
        if unsafe { setrlimit(RLIMIT_CPU, &raw const limit) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    };
    // SAFETY: the callback uses only setrlimit with a stack value and performs no allocation,
    // locking, or access to mutable parent state after fork.
    unsafe {
        command.pre_exec(set_limit);
    }
    Ok(())
}

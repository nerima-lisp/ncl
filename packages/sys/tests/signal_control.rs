//! Integration tests for the OS signal boundary.

use core::ffi::{c_int, c_void};
use ncl_sys::Thread;

unsafe extern "C" {
    fn raise(signal: c_int) -> c_int;
    fn pipe(descriptors: *mut c_int) -> c_int;
    fn close(descriptor: c_int) -> c_int;
    fn write(descriptor: c_int, buffer: *const c_void, count: usize) -> isize;
    fn getpagesize() -> c_int;
}

const SIG_INT: c_int = 2;

#[test]
fn blocked_control_signal_is_consumed_by_sigwait() -> Result<(), Box<dyn std::error::Error>> {
    let signal = match std::thread::spawn(|| -> std::io::Result<c_int> {
        Thread::block_signal_control_set()?;
        // SAFETY: SIGINT is blocked on this thread, so raise leaves it pending for sigwait.
        if unsafe { raise(SIG_INT) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        wait_control_signal()
    })
    .join()
    {
        Ok(result) => result?,
        Err(_) => return Err(std::io::Error::other("signal test thread panicked").into()),
    };
    assert_eq!(signal, SIG_INT);
    Ok(())
}

#[cfg(target_os = "linux")]
fn wait_control_signal() -> std::io::Result<c_int> {
    Thread::wait_signal_control_set()
}

#[cfg(target_os = "macos")]
fn wait_control_signal() -> std::io::Result<c_int> {
    Thread::wait_process_signal()
}

#[test]
fn alternate_signal_stack_meets_contract_size_and_is_reused()
-> Result<(), Box<dyn std::error::Error>> {
    let first = Thread::ensure_alt_signal_stack()?;
    let second = Thread::ensure_alt_signal_stack()?;
    // SAFETY: getpagesize has no pointer preconditions.
    let page_size = unsafe { getpagesize() };
    assert!(page_size > 0);
    assert_eq!(first, second);
    assert_eq!(first % usize::try_from(page_size)?, 0);
    #[cfg(target_os = "linux")]
    assert!(first >= 8 * 1024 + 64 * 1024);
    #[cfg(target_os = "macos")]
    assert!(first >= 128 * 1024 + 64 * 1024);
    Ok(())
}

#[test]
fn sigpipe_is_ignored_and_closed_pipe_write_returns_broken_pipe()
-> Result<(), Box<dyn std::error::Error>> {
    Thread::ignore_sigpipe()?;
    let mut descriptors = [-1; 2];
    // SAFETY: descriptors points to two writable C integers for pipe's result.
    assert_eq!(unsafe { pipe(descriptors.as_mut_ptr()) }, 0);
    // SAFETY: descriptors[0] is the read end returned by pipe and remains open here.
    assert_eq!(unsafe { close(descriptors[0]) }, 0);
    let byte = *b"x";
    // SAFETY: descriptors[1] is the live write end and byte provides one readable byte.
    let result = unsafe { write(descriptors[1], byte.as_ptr().cast(), byte.len()) };
    assert_eq!(result, -1);
    assert_eq!(
        std::io::Error::last_os_error().kind(),
        std::io::ErrorKind::BrokenPipe
    );
    // SAFETY: descriptors[1] is the write end returned by pipe and remains open.
    assert_eq!(unsafe { close(descriptors[1]) }, 0);
    Ok(())
}

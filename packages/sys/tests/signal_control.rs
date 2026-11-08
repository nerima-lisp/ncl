//! Integration tests for the OS signal boundary.

use core::ffi::{c_int, c_void};
use ncl_sys::Thread;
use std::cell::{Cell, RefCell};
use std::io;

#[path = "../src/os/signal_control/alternate_stack.rs"]
mod alternate_stack;
#[path = "../src/os/signal_control/disposition.rs"]
mod disposition;
#[path = "../src/os/signal_control/sigwait.rs"]
mod sigwait;

unsafe extern "C" {
    fn raise(signal: c_int) -> c_int;
    fn pipe(descriptors: *mut c_int) -> c_int;
    fn close(descriptor: c_int) -> c_int;
    fn write(descriptor: c_int, buffer: *const c_void, count: usize) -> isize;
    fn getpagesize() -> c_int;
}

const SIG_INT: c_int = 2;
#[cfg(target_os = "linux")]
const SIG_CHLD: c_int = 17;
#[cfg(target_os = "macos")]
const SIG_CHLD: c_int = 20;

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

#[cfg(target_os = "linux")]
fn wait_child_signal() -> std::io::Result<c_int> {
    Thread::wait_signal_control_set()
}

#[cfg(target_os = "macos")]
fn wait_child_signal() -> std::io::Result<c_int> {
    Thread::wait_child_signal()
}

#[test]
fn alternate_signal_stack_meets_contract_size_and_is_reused()
-> Result<(), Box<dyn std::error::Error>> {
    let (first, second) = std::thread::spawn(|| -> io::Result<_> {
        let first = Thread::ensure_alt_signal_stack()?;
        let second = Thread::ensure_alt_signal_stack()?;
        Ok((first, second))
    })
    .join()
    .map_err(|_| io::Error::other("alternate-stack test thread panicked"))??;
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

#[test]
fn sigchld_disposition_handles_unblocked_and_blocked_signals()
-> Result<(), Box<dyn std::error::Error>> {
    let signal = std::thread::spawn(|| -> io::Result<c_int> {
        Thread::install_sigchld_wait_disposition()?;
        // SAFETY: SIGCHLD has the installed no-op disposition, so an unblocked raise returns.
        if unsafe { raise(SIG_CHLD) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Thread::block_signal_control_set()?;
        // SAFETY: SIGCHLD is blocked here and stays pending for the following sigwait.
        if unsafe { raise(SIG_CHLD) } != 0 {
            return Err(io::Error::last_os_error());
        }
        wait_child_signal()
    })
    .join()
    .map_err(|_| io::Error::other("SIGCHLD test thread panicked"))??;
    assert_eq!(signal, SIG_CHLD);
    Ok(())
}

#[test]
fn signal_set_creation_reports_empty_and_add_failures() {
    let added_after_empty_failure = Cell::new(false);
    let empty_error = disposition::create_signal_set(
        &[SIG_INT],
        Vec::<c_int>::new,
        |_| -1,
        |_, _| {
            added_after_empty_failure.set(true);
            0
        },
        || io::Error::from_raw_os_error(5),
    )
    .expect_err("empty-set failure should be returned");
    assert_eq!(empty_error.raw_os_error(), Some(5));
    assert!(!added_after_empty_failure.get());

    let add_error = disposition::create_signal_set(
        &[SIG_INT, 15],
        Vec::new,
        |_| 0,
        |set, signal| {
            if signal == 15 {
                -1
            } else {
                set.push(signal);
                0
            }
        },
        || io::Error::from_raw_os_error(22),
    )
    .expect_err("signal-add failure should be returned");
    assert_eq!(add_error.raw_os_error(), Some(22));

    let set = disposition::create_signal_set(
        &[SIG_INT, 15],
        Vec::new,
        |_| 0,
        |set, signal| {
            set.push(signal);
            0
        },
        io::Error::last_os_error,
    )
    .expect("valid signals should be added");
    assert_eq!(set, [SIG_INT, 15]);
}

#[test]
fn disposition_setup_reports_mask_and_install_failures() {
    let mut action = ();
    let installed_after_mask_failure = Cell::new(false);
    let mask_error = disposition::install_signal_disposition(
        &mut action,
        |_| -1,
        |_| {
            installed_after_mask_failure.set(true);
            0
        },
        || io::Error::from_raw_os_error(5),
    )
    .expect_err("mask setup failure should be returned");
    assert_eq!(mask_error.raw_os_error(), Some(5));
    assert!(!installed_after_mask_failure.get());

    let install_error = disposition::install_signal_disposition(
        &mut action,
        |_| 0,
        |_| -1,
        || io::Error::from_raw_os_error(22),
    )
    .expect_err("sigaction failure should be returned");
    assert_eq!(install_error.raw_os_error(), Some(22));

    disposition::install_signal_disposition(&mut action, |_| 0, |_| 0, io::Error::last_os_error)
        .expect("valid disposition should install");
    disposition::block_control_signal_mask(0).expect("pthread_sigmask success should be preserved");
    assert_eq!(
        disposition::block_control_signal_mask(22)
            .expect_err("pthread_sigmask returns its error number")
            .raw_os_error(),
        Some(22)
    );
}

#[test]
fn sigwait_retries_interruption_and_returns_terminal_results() {
    let mut calls = 0;
    let signal = sigwait::wait_signal_set(&(), |_, signal| {
        calls += 1;
        if calls == 1 {
            4
        } else {
            *signal = SIG_INT;
            0
        }
    })
    .expect("sigwait should retry EINTR");
    assert_eq!(signal, SIG_INT);
    assert_eq!(calls, 2);

    let error = sigwait::wait_signal_set(&(), |_, _| 22)
        .expect_err("non-interrupted wait errors should be returned");
    assert_eq!(error.raw_os_error(), Some(22));
}

#[test]
fn alternate_stack_size_checks_sysconf_page_and_overflow_values() {
    #[cfg(target_os = "linux")]
    {
        assert_eq!(alternate_stack::positive_sysconf(4096, 256), 4096);
        assert_eq!(alternate_stack::positive_sysconf(0, 256), 256);
        assert_eq!(alternate_stack::positive_sysconf(-1, 256), 256);
    }

    assert_eq!(
        alternate_stack::required_alt_signal_stack_size(5001, 6000, 4096, 100)
            .expect("valid stack sizes should be rounded"),
        8292
    );
    assert_eq!(
        alternate_stack::required_alt_signal_stack_size(1, 1, 0, 0)
            .expect_err("zero page size is invalid")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        alternate_stack::required_alt_signal_stack_size(usize::MAX, 1, 2, 0)
            .expect_err("page rounding must detect overflow")
            .kind(),
        io::ErrorKind::Other
    );
    assert_eq!(
        alternate_stack::required_alt_signal_stack_size(1, 1, 1, usize::MAX)
            .expect_err("reserve addition must detect overflow")
            .kind(),
        io::ErrorKind::Other
    );
}

#[test]
fn alternate_stack_query_and_prepare_paths_preserve_errors() {
    alternate_stack::query_alternate_signal_stack(0, io::Error::last_os_error)
        .expect("successful query should be accepted");
    assert_eq!(
        alternate_stack::query_alternate_signal_stack(-1, || io::Error::from_raw_os_error(5))
            .expect_err("failed query should report the OS error")
            .raw_os_error(),
        Some(5)
    );

    let mut attempted_registration = false;
    let existing = alternate_stack::ensure_alternate_signal_stack(4096, 8192, 0, 2, 1, |_| {
        attempted_registration = true;
        Ok(0)
    })
    .expect("adequate stack should be reused");
    assert_eq!(existing, 8192);
    assert!(!attempted_registration);

    assert_eq!(
        alternate_stack::ensure_alternate_signal_stack(4096, 0, 1, 2, 1, |_| Ok(0))
            .expect_err("an active undersized stack cannot be replaced")
            .raw_os_error(),
        Some(1)
    );

    let mut registered_size = 0;
    assert_eq!(
        alternate_stack::ensure_alternate_signal_stack(4096, 0, 2, 2, 1, |required| {
            registered_size = required;
            Ok(required)
        })
        .expect("disabled stack should be replaced"),
        4096
    );
    assert_eq!(registered_size, 4096);

    let register_error = alternate_stack::ensure_alternate_signal_stack(4096, 0, 2, 2, 1, |_| {
        Err(io::Error::from_raw_os_error(12))
    })
    .expect_err("registration failures should be returned");
    assert_eq!(register_error.raw_os_error(), Some(12));
}

#[test]
fn alternate_stack_registration_handles_borrow_and_os_failures() {
    let owner = RefCell::new(alternate_stack::AltSignalStackOwner { memory: None });
    let size = alternate_stack::register_alternate_signal_stack(&owner, 8192, |pointer, length| {
        assert!(!pointer.is_null());
        assert_eq!(length, 8192);
        Ok(())
    })
    .expect("stack registration should store its backing allocation");
    assert_eq!(size, 8192);
    assert_eq!(
        owner.borrow().memory.as_ref().map(|memory| memory.len()),
        Some(8192)
    );

    let failed_owner = RefCell::new(alternate_stack::AltSignalStackOwner { memory: None });
    assert_eq!(
        alternate_stack::register_alternate_signal_stack(&failed_owner, 1024, |_, _| {
            Err(io::Error::from_raw_os_error(12))
        })
        .expect_err("registration failure should be returned")
        .raw_os_error(),
        Some(12)
    );
    assert!(failed_owner.borrow().memory.is_none());

    let held = failed_owner.borrow_mut();
    assert_eq!(
        alternate_stack::register_alternate_signal_stack(&failed_owner, 1024, |_, _| Ok(()))
            .expect_err("recursive registration must not borrow the owner")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(held);
}

#[test]
fn alternate_stack_tls_and_release_guards_keep_memory_safe() {
    assert_eq!(
        alternate_stack::map_alternate_signal_stack_tls::<_, ()>(Ok(Ok(7)))
            .expect("available TLS value should pass through"),
        7
    );
    assert_eq!(
        alternate_stack::map_alternate_signal_stack_tls::<usize, ()>(Ok(Err(
            io::Error::from_raw_os_error(12),
        )))
        .expect_err("TLS closure errors should be returned")
        .raw_os_error(),
        Some(12)
    );
    assert_eq!(
        alternate_stack::map_alternate_signal_stack_tls::<usize, _>(Err(()))
            .expect_err("TLS access failure should be surfaced")
            .kind(),
        io::ErrorKind::Other
    );

    let queried = Cell::new(false);
    let disabled = Cell::new(false);
    let retained_size = Cell::new(0);
    alternate_stack::release_alternate_signal_stack(
        None,
        || {
            queried.set(true);
            Ok((0, 0))
        },
        2,
        1,
        || {
            disabled.set(true);
            Ok(())
        },
        |memory| retained_size.set(memory.len()),
    );
    assert!(!queried.get());
    assert!(!disabled.get());
    assert_eq!(retained_size.get(), 0);

    let memory = vec![0_u8; 8].into_boxed_slice();
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Err(io::Error::from_raw_os_error(5)),
        2,
        1,
        || Ok(()),
        |memory| retained_size.set(memory.len()),
    );
    assert_eq!(retained_size.get(), 8);

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr() as usize;
    retained_size.set(0);
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Ok((address + 1, 0)),
        2,
        1,
        || {
            disabled.set(true);
            Ok(())
        },
        |memory| retained_size.set(memory.len()),
    );
    assert!(!disabled.get());
    assert_eq!(retained_size.get(), 0);

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr() as usize;
    retained_size.set(0);
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Ok((address, 1)),
        2,
        1,
        || Ok(()),
        |memory| retained_size.set(memory.len()),
    );
    assert_eq!(retained_size.get(), 8);

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr() as usize;
    retained_size.set(0);
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Ok((address, 0)),
        2,
        1,
        || Err(io::Error::from_raw_os_error(5)),
        |memory| retained_size.set(memory.len()),
    );
    assert_eq!(retained_size.get(), 8);

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr() as usize;
    retained_size.set(0);
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Ok((address, 2)),
        2,
        1,
        || {
            disabled.set(true);
            Ok(())
        },
        |memory| retained_size.set(memory.len()),
    );
    assert!(!disabled.get());
    assert_eq!(retained_size.get(), 0);

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr() as usize;
    disabled.set(false);
    alternate_stack::release_alternate_signal_stack(
        Some(memory),
        || Ok((address, 0)),
        2,
        1,
        || {
            disabled.set(true);
            Ok(())
        },
        |memory| retained_size.set(memory.len()),
    );
    assert!(disabled.get());
    assert_eq!(retained_size.get(), 0);
}

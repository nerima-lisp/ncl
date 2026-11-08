#[cfg(target_os = "macos")]
use super::SIG_CHLD;
use super::{
    ALT_SIGNAL_STACK, AltSignalStackOwner, SIG_INT, alternate_stack, block_signal_set,
    current_alt_signal_stack, disposition, required_alt_signal_stack_size, sigwait,
    wait_signal_set,
};
use core::ffi::c_int;
use std::cell::{Cell, RefCell};
use std::io;

unsafe extern "C" {
    fn raise(signal: c_int) -> c_int;
}

fn assert_raw_os_error<T>(result: io::Result<T>, code: c_int) {
    assert!(matches!(result, Err(error) if error.raw_os_error() == Some(code)));
}

fn assert_io_error_kind<T>(result: io::Result<T>, kind: io::ErrorKind) {
    assert!(matches!(result, Err(error) if error.kind() == kind));
}

#[test]
fn signal_set_creation_reports_empty_and_add_failures() {
    let added_after_empty_failure = Cell::new(false);
    assert_raw_os_error(
        disposition::create_signal_set(
            &[SIG_INT],
            Vec::<c_int>::new,
            |_| -1,
            |_, _| {
                added_after_empty_failure.set(true);
                0
            },
            || io::Error::from_raw_os_error(5),
        ),
        5,
    );
    assert!(!added_after_empty_failure.get());

    assert_raw_os_error(
        disposition::create_signal_set(
            &[SIG_INT, 15],
            Vec::<c_int>::new,
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
        ),
        22,
    );

    let set = disposition::create_signal_set(
        &[SIG_INT, 15],
        Vec::<c_int>::new,
        |_| 0,
        |set, signal| {
            set.push(signal);
            0
        },
        io::Error::last_os_error,
    );
    assert!(matches!(set, Ok(set) if set == [SIG_INT, 15]));
}

#[test]
fn disposition_setup_reports_mask_and_install_failures() {
    let mut action = Vec::<u8>::new();
    let installed_after_mask_failure = Cell::new(false);
    assert_raw_os_error(
        disposition::install_signal_disposition(
            &mut action,
            |_| -1,
            |_| {
                installed_after_mask_failure.set(true);
                0
            },
            || io::Error::from_raw_os_error(5),
        ),
        5,
    );
    assert!(!installed_after_mask_failure.get());

    assert_raw_os_error(
        disposition::install_signal_disposition(
            &mut action,
            |_| 0,
            |_| -1,
            || io::Error::from_raw_os_error(22),
        ),
        22,
    );

    assert!(
        disposition::install_signal_disposition(
            &mut action,
            |_| 0,
            |_| 0,
            io::Error::last_os_error
        )
        .is_ok()
    );
    assert!(disposition::block_control_signal_mask(0).is_ok());
    assert_raw_os_error(disposition::block_control_signal_mask(22), 22);
}

#[test]
fn sigwait_retries_interruption_and_returns_terminal_results() {
    let mut calls = 0;
    let set = 0_u8;
    let signal = sigwait::wait_signal_set(&set, |_, signal| {
        calls += 1;
        if calls == 1 {
            4
        } else {
            *signal = SIG_INT;
            0
        }
    });
    assert!(matches!(signal, Ok(SIG_INT)));
    assert_eq!(calls, 2);

    assert_raw_os_error(sigwait::wait_signal_set(&set, |_, _| 22), 22);
}

#[test]
fn alternate_stack_size_checks_sysconf_page_and_overflow_values() {
    #[cfg(target_os = "linux")]
    {
        assert_eq!(alternate_stack::positive_sysconf(4096, 256), 4096);
        assert_eq!(alternate_stack::positive_sysconf(0, 256), 256);
        assert_eq!(alternate_stack::positive_sysconf(-1, 256), 256);
    }

    assert!(matches!(
        alternate_stack::required_alt_signal_stack_size(5001, 6000, 4096, 100),
        Ok(8292)
    ));
    assert_io_error_kind(
        alternate_stack::required_alt_signal_stack_size(1, 1, 0, 0),
        io::ErrorKind::InvalidData,
    );
    assert_io_error_kind(
        alternate_stack::required_alt_signal_stack_size(usize::MAX, 1, 2, 0),
        io::ErrorKind::Other,
    );
    assert_io_error_kind(
        alternate_stack::required_alt_signal_stack_size(1, 1, 1, usize::MAX),
        io::ErrorKind::Other,
    );
}

#[test]
fn alternate_stack_query_and_prepare_paths_preserve_errors() {
    assert!(alternate_stack::query_alternate_signal_stack(0, io::Error::last_os_error).is_ok());
    assert_raw_os_error(
        alternate_stack::query_alternate_signal_stack(-1, || io::Error::from_raw_os_error(5)),
        5,
    );

    let mut attempted_registration = false;
    let existing = alternate_stack::ensure_alternate_signal_stack(4096, 8192, 0, 2, 1, |_| {
        attempted_registration = true;
        Ok(0)
    });
    assert!(matches!(existing, Ok(8192)));
    assert!(!attempted_registration);

    assert_raw_os_error(
        alternate_stack::ensure_alternate_signal_stack(4096, 0, 1, 2, 1, |_| Ok(0)),
        1,
    );

    let mut registered_size = 0;
    let replaced = alternate_stack::ensure_alternate_signal_stack(4096, 0, 2, 2, 1, |required| {
        registered_size = required;
        Ok(required)
    });
    assert!(matches!(replaced, Ok(4096)));
    assert_eq!(registered_size, 4096);

    assert_raw_os_error(
        alternate_stack::ensure_alternate_signal_stack(4096, 0, 2, 2, 1, |_| {
            Err(io::Error::from_raw_os_error(12))
        }),
        12,
    );
}

#[test]
fn alternate_stack_registration_handles_borrow_and_os_failures() {
    let owner = RefCell::new(alternate_stack::AltSignalStackOwner { memory: None });
    let size = alternate_stack::register_alternate_signal_stack(&owner, 8192, |pointer, length| {
        assert!(!pointer.is_null());
        assert_eq!(length, 8192);
        Ok(())
    });
    assert!(matches!(size, Ok(8192)));
    assert_eq!(
        owner.borrow().memory.as_ref().map(|memory| memory.len()),
        Some(8192)
    );

    let failed_owner = RefCell::new(alternate_stack::AltSignalStackOwner { memory: None });
    assert_raw_os_error(
        alternate_stack::register_alternate_signal_stack(&failed_owner, 1024, |_, _| {
            Err(io::Error::from_raw_os_error(12))
        }),
        12,
    );
    assert!(failed_owner.borrow().memory.is_none());

    let held = failed_owner.borrow_mut();
    assert_io_error_kind(
        alternate_stack::register_alternate_signal_stack(&failed_owner, 1024, |_, _| Ok(())),
        io::ErrorKind::WouldBlock,
    );
    drop(held);
}

#[test]
fn alternate_stack_tls_result_preserves_inner_and_outer_errors() {
    assert!(matches!(
        alternate_stack::map_alternate_signal_stack_tls::<_, ()>(Ok(Ok(7))),
        Ok(7)
    ));
    assert_raw_os_error(
        alternate_stack::map_alternate_signal_stack_tls::<usize, ()>(Ok(Err(
            io::Error::from_raw_os_error(12),
        ))),
        12,
    );
    assert_io_error_kind(
        alternate_stack::map_alternate_signal_stack_tls::<usize, _>(Err(())),
        io::ErrorKind::Other,
    );
}

fn release_stack_for_test(
    memory: Option<Box<[u8]>>,
    current: impl FnOnce() -> io::Result<(usize, c_int)>,
    disable: impl FnOnce() -> io::Result<()>,
) -> (usize, bool) {
    let retained_size = Cell::new(0);
    let disable_called = Cell::new(false);
    alternate_stack::release_alternate_signal_stack(
        memory,
        current,
        2,
        1,
        || {
            disable_called.set(true);
            disable()
        },
        |memory| retained_size.set(memory.len()),
    );
    (retained_size.get(), disable_called.get())
}

#[test]
fn alternate_stack_release_retains_memory_when_stack_may_be_active() {
    let memory = vec![0_u8; 8].into_boxed_slice();
    assert_eq!(
        release_stack_for_test(
            Some(memory),
            || Err(io::Error::from_raw_os_error(5)),
            || Ok(()),
        ),
        (8, false)
    );

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr().addr();
    assert_eq!(
        release_stack_for_test(Some(memory), || Ok((address, 1)), || Ok(())),
        (8, false)
    );

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr().addr();
    assert_eq!(
        release_stack_for_test(
            Some(memory),
            || Ok((address, 0)),
            || Err(io::Error::from_raw_os_error(5)),
        ),
        (8, true)
    );
}

#[test]
fn alternate_stack_release_discards_unowned_or_disabled_memory() {
    let queried = Cell::new(false);
    assert_eq!(
        release_stack_for_test(
            None,
            || {
                queried.set(true);
                Ok((0, 0))
            },
            || Ok(()),
        ),
        (0, false)
    );
    assert!(!queried.get());

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr().addr();
    assert_eq!(
        release_stack_for_test(Some(memory), || Ok((address + 1, 0)), || Ok(())),
        (0, false)
    );

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr().addr();
    assert_eq!(
        release_stack_for_test(Some(memory), || Ok((address, 2)), || Ok(())),
        (0, false)
    );

    let memory = vec![0_u8; 8].into_boxed_slice();
    let address = memory.as_ptr().addr();
    assert_eq!(
        release_stack_for_test(Some(memory), || Ok((address, 0)), || Ok(())),
        (0, true)
    );
}

#[test]
fn wait_and_mask_helpers_reject_invalid_signal_numbers() {
    assert!(matches!(
        wait_signal_set(&[c_int::MAX]),
        Err(error) if error.raw_os_error().is_some()
    ));
    assert!(matches!(
        block_signal_set(&[c_int::MAX]),
        Err(error) if error.raw_os_error().is_some()
    ));
}

#[test]
fn dropping_current_thread_stack_disables_the_registered_alternate_signal_stack() {
    let worker = std::thread::spawn(|| -> io::Result<(usize, bool, bool, i32)> {
        let size = crate::Thread::ensure_alt_signal_stack()?;
        let owner = ALT_SIGNAL_STACK
            .try_with(|owner| {
                let mut owner = owner.try_borrow_mut().map_err(|_| {
                    io::Error::new(io::ErrorKind::WouldBlock, "alternate stack is borrowed")
                })?;
                Ok::<_, io::Error>(std::mem::replace(
                    &mut *owner,
                    AltSignalStackOwner { memory: None },
                ))
            })
            .map_err(|_| io::Error::other("alternate stack TLS unavailable"))??;
        let owns_stack = owner.memory.is_some();
        let before = current_alt_signal_stack()?;
        let pointer_matches = owner
            .memory
            .as_ref()
            .is_some_and(|memory| memory.as_ptr().addr() == before.sp.addr());
        drop(owner);
        let current = current_alt_signal_stack()?;
        Ok((size, owns_stack, pointer_matches, current.flags))
    });
    let result = worker.join();
    assert!(
        matches!(result, Ok(Ok((size, true, true, flags))) if size >= 64 * 1024 && flags & super::SS_DISABLE != 0),
        "alternate stack cleanup result: {result:?}"
    );
}

#[test]
fn current_thread_uses_and_reuses_a_contract_sized_alternate_signal_stack() {
    assert!(matches!(
        required_alt_signal_stack_size(),
        Ok(size) if size >= 64 * 1024
    ));
    let first = crate::Thread::ensure_alt_signal_stack();
    let second = crate::Thread::ensure_alt_signal_stack();
    assert!(matches!(
        (first, second),
        (Ok(first), Ok(second)) if first >= 64 * 1024 && first == second
    ));
    assert!(matches!(
        current_alt_signal_stack(),
        Ok(stack) if stack.size >= 64 * 1024 && stack.flags & super::SS_DISABLE == 0
    ));
}

#[test]
fn dropping_an_unregistered_stack_preserves_the_current_stack() {
    let before = current_alt_signal_stack();
    assert!(before.is_ok());
    let Ok(before) = before else { return };
    drop(AltSignalStackOwner {
        memory: Some(vec![0_u8; 8].into_boxed_slice()),
    });
    assert!(matches!(
        current_alt_signal_stack(),
        Ok(after)
            if after.sp.addr() == before.sp.addr()
                && after.size == before.size
                && after.flags == before.flags
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_control_mask_and_sigwait_wrappers_consume_sigint() {
    let worker = std::thread::spawn(|| -> io::Result<c_int> {
        crate::Thread::block_signal_control_set()?;
        // SAFETY: SIGINT is blocked on this thread, so raise queues it for sigwait.
        if unsafe { raise(SIG_INT) } != 0 {
            return Err(io::Error::last_os_error());
        }
        crate::Thread::wait_signal_control_set()
    });
    assert!(matches!(worker.join(), Ok(Ok(SIG_INT))));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_process_wait_wrapper_consumes_sigint() {
    let worker = std::thread::spawn(|| -> io::Result<c_int> {
        crate::Thread::block_signal_control_set()?;
        // SAFETY: SIGINT is blocked on this thread, so raise queues it for sigwait.
        if unsafe { raise(SIG_INT) } != 0 {
            return Err(io::Error::last_os_error());
        }
        crate::Thread::wait_process_signal()
    });
    assert!(matches!(worker.join(), Ok(Ok(SIG_INT))));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_child_wait_wrapper_consumes_sigchld() {
    assert!(crate::Thread::install_sigchld_wait_disposition().is_ok());
    let worker = std::thread::spawn(|| -> io::Result<c_int> {
        crate::Thread::block_signal_control_set()?;
        // SAFETY: SIGCHLD is blocked on this thread, so raise queues it for sigwait.
        if unsafe { raise(SIG_CHLD) } != 0 {
            return Err(io::Error::last_os_error());
        }
        crate::Thread::wait_child_signal()
    });
    assert!(matches!(worker.join(), Ok(Ok(SIG_CHLD))));
}

#[test]
fn sigpipe_and_sigchld_disposition_wrappers_succeed() {
    assert!(crate::Thread::ignore_sigpipe().is_ok());
    assert!(crate::Thread::install_sigchld_wait_disposition().is_ok());
}

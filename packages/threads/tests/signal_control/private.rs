use super::*;
use std::error::Error as _;

fn control_with_result(result: Result<SignalEvent, SignalControlError>) -> SignalControl {
    let (sender, receiver) = mpsc::channel();
    assert!(sender.send(result).is_ok());
    SignalControl {
        receiver,
        failed: Cell::new(false),
    }
}

fn disconnected_control() -> SignalControl {
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    SignalControl {
        receiver,
        failed: Cell::new(false),
    }
}

fn failed_wait() -> io::Result<i32> {
    Err(io::Error::other("simulated sigwait failure"))
}

#[test]
fn signal_control_errors_have_stable_display_and_sources() {
    let errors = [
        (
            SignalControlError::OperatingSystem(io::Error::other("source")),
            "signal setup failed: source",
            Some("source"),
        ),
        (
            SignalControlError::ThreadSpawn(io::Error::other("source")),
            "signal control thread creation failed: source",
            Some("source"),
        ),
        (
            SignalControlError::AlreadyStarted,
            "signal control thread is already started",
            None,
        ),
        (
            SignalControlError::SignalWait(io::Error::other("source")),
            "sigwait failed: source",
            Some("source"),
        ),
        (
            SignalControlError::UnexpectedSignal(-1),
            "sigwait returned unexpected signal -1",
            None,
        ),
        (
            SignalControlError::WorkerStopped,
            "signal control thread stopped",
            None,
        ),
        (
            SignalControlError::StartStatePoisoned,
            "signal control startup state is poisoned",
            None,
        ),
    ];
    for (error, display, expected_source) in errors {
        assert_eq!(error.to_string(), display);
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            expected_source
        );
    }
}

#[test]
fn signal_control_debug_reports_its_public_type() {
    let control = control_with_result(Ok(SignalEvent::Interrupt));
    assert_eq!(format!("{control:?}"), "SignalControl { .. }");
}

#[test]
fn startup_failures_keep_typed_errors() {
    assert!(matches!(
        map_signal_setup_result(Err(io::Error::other("setup failure"))),
        Err(SignalControlError::OperatingSystem(error))
            if error.to_string() == "setup failure"
    ));
    assert!(matches!(
        map_signal_worker_spawn(Err(io::Error::other("spawn failure"))),
        Err(SignalControlError::ThreadSpawn(error))
            if error.to_string() == "spawn failure"
    ));
    let state = Mutex::new(false);
    let guard = match state.lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    assert!(matches!(
        lock_signal_control_start_state(Err(PoisonError::new(guard))),
        Err(SignalControlError::StartStatePoisoned)
    ));
}

#[test]
fn control_event_maps_configured_signals_and_rejects_unknown_values() {
    assert_eq!(control_event(2), Some(SignalEvent::Interrupt));
    assert_eq!(control_event(15), Some(SignalEvent::TerminationRequested));
    #[cfg(target_os = "linux")]
    assert_eq!(control_event(17), Some(SignalEvent::ChildrenChanged));
    #[cfg(target_os = "macos")]
    assert_eq!(control_event(20), Some(SignalEvent::ChildrenChanged));
    assert_eq!(control_event(-1), None);
}

#[test]
fn worker_failure_stops_siblings_and_receiver() {
    let worker_failed = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let mut wait = failed_wait;
    sigwait_loop(&sender, &worker_failed, &mut wait);
    assert!(worker_failed.load(Ordering::Acquire));
    let mut sibling_wait_calls = 0;
    let mut sibling_wait = || {
        sibling_wait_calls += 1;
        Ok(2)
    };
    sigwait_loop(&sender, &worker_failed, &mut sibling_wait);
    assert_eq!(sibling_wait_calls, 0);
    let control = SignalControl {
        receiver,
        failed: Cell::new(false),
    };
    assert!(matches!(
        control.receive(),
        Err(SignalControlError::SignalWait(_))
    ));
    assert!(matches!(
        control.receive(),
        Err(SignalControlError::WorkerStopped)
    ));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Err(SignalControlError::WorkerStopped)
    ));
    assert!(matches!(
        control.try_receive(),
        Err(SignalControlError::WorkerStopped)
    ));
}

#[test]
fn wait_failure_before_return_stops_without_reporting_an_event() {
    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    let mut wait_calls = 0;
    let mut wait = || {
        wait_calls += 1;
        worker_failed.store(true, Ordering::Release);
        Ok(2)
    };
    sigwait_loop(&sender, &worker_failed, &mut wait);
    assert_eq!(wait_calls, 1);
    assert!(worker_failed.load(Ordering::Acquire));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn unexpected_signal_is_reported_as_worker_failure() {
    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    let mut wait = || Ok(-1);
    sigwait_loop(&sender, &worker_failed, &mut wait);
    assert!(worker_failed.load(Ordering::Acquire));
    assert!(matches!(
        receiver.try_recv(),
        Ok(Err(SignalControlError::UnexpectedSignal(-1)))
    ));
}

#[test]
fn event_send_failure_stops_the_worker() {
    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    let mut wait_calls = 0;
    let mut wait = || {
        wait_calls += 1;
        if wait_calls == 1 {
            Ok(2)
        } else {
            Err(io::Error::other("wait continued after receiver disconnect"))
        }
    };
    sigwait_loop(&sender, &worker_failed, &mut wait);
    assert_eq!(wait_calls, 1);
    assert!(!worker_failed.load(Ordering::Acquire));
}

#[test]
fn disconnected_receiver_is_terminal_for_each_receive_method() {
    assert!(matches!(
        disconnected_control().receive(),
        Err(SignalControlError::WorkerStopped)
    ));
    assert!(matches!(
        disconnected_control().receive_timeout(Duration::ZERO),
        Err(SignalControlError::WorkerStopped)
    ));
    assert!(matches!(
        disconnected_control().try_receive(),
        Err(SignalControlError::WorkerStopped)
    ));
}

#[test]
fn receive_timeout_returns_none_when_the_queue_stays_empty() {
    let (sender, receiver) = mpsc::channel();
    let control = SignalControl {
        receiver,
        failed: Cell::new(false),
    };
    assert!(matches!(control.receive_timeout(Duration::ZERO), Ok(None)));
    assert!(!control.failed.get());
    drop(sender);
}

#[test]
fn try_receive_returns_none_when_the_queue_is_empty() {
    let (sender, receiver) = mpsc::channel();
    let control = SignalControl {
        receiver,
        failed: Cell::new(false),
    };
    assert!(matches!(control.try_receive(), Ok(None)));
    assert!(!control.failed.get());
    drop(sender);
}

#[test]
fn receive_methods_deliver_queued_events_without_marking_failure() {
    let control = control_with_result(Ok(SignalEvent::Interrupt));
    assert!(matches!(control.receive(), Ok(SignalEvent::Interrupt)));
    assert!(!control.failed.get());
    let control = control_with_result(Ok(SignalEvent::TerminationRequested));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Ok(Some(SignalEvent::TerminationRequested))
    ));
    assert!(!control.failed.get());
    let control = control_with_result(Ok(SignalEvent::ChildrenChanged));
    assert!(matches!(
        control.try_receive(),
        Ok(Some(SignalEvent::ChildrenChanged))
    ));
    assert!(!control.failed.get());
}

#[test]
fn receive_methods_return_the_first_worker_error() {
    let control = control_with_result(Err(SignalControlError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(
        control.receive(),
        Err(SignalControlError::SignalWait(_))
    ));
    let control = control_with_result(Err(SignalControlError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Err(SignalControlError::SignalWait(_))
    ));
    let control = control_with_result(Err(SignalControlError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(
        control.try_receive(),
        Err(SignalControlError::SignalWait(_))
    ));
}

const START_CHILD_MARKER: &str = "NCL_SIGNAL_CONTROL_START_CHILD";

#[test]
fn signal_control_starts_and_receives_interrupt_in_a_subprocess()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::thread::spawn(|| -> io::Result<_> {
        ncl_sys::Thread::block_signal_control_set()?;
        std::process::Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "signal_control::private_tests::signal_control_start_child",
                "--nocapture",
            ])
            .env(START_CHILD_MARKER, "1")
            .output()
    })
    .join();
    let output = match output {
        Ok(output) => output?,
        Err(_) => return Err(io::Error::other("signal child launcher panicked").into()),
    };
    assert!(
        output.status.success(),
        "signal control child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
#[ignore = "run in a subprocess to isolate process-wide signal setup"]
fn signal_control_start_child() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(START_CHILD_MARKER).is_none() {
        return Ok(());
    }
    let control = SignalControl::start()?;
    SignalControl::prepare_current_thread()?;
    let process_id = std::process::id().to_string();
    let status = std::process::Command::new("kill")
        .args(["-INT", &process_id])
        .status()?;
    assert!(status.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        if timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "sigwait worker did not receive SIGINT",
            )
            .into());
        }
        match control.receive_timeout(timeout)? {
            Some(SignalEvent::Interrupt) => break,
            Some(SignalEvent::ChildrenChanged) => {}
            Some(event) => {
                return Err(io::Error::other(format!(
                    "unexpected signal event before SIGINT: {event:?}"
                ))
                .into());
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "sigwait worker did not receive SIGINT",
                )
                .into());
            }
        }
    }
    Ok(())
}

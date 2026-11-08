//! Integration tests for the runtime signal event queue.

use ncl_threads::signal_control::SignalControlError;
use ncl_threads::signal_control::{SignalControl, SignalEvent};
use std::error::Error as _;
use std::io;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

#[path = "../src/signal_control.rs"]
mod signal_control_source;

const CHILD_MARKER: &str = "NCL_SIGWAIT_CHILD";
const SOURCE_CHILD_MARKER: &str = "NCL_SIGNAL_CONTROL_SOURCE_CHILD";

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

fn source_control_with_result(
    result: Result<signal_control_source::SignalEvent, signal_control_source::SignalControlError>,
) -> signal_control_source::SignalControl {
    let (sender, receiver) = mpsc::channel();
    assert!(sender.send(result).is_ok());
    signal_control_source::SignalControl::with_receiver(receiver)
}

fn source_disconnected_control() -> signal_control_source::SignalControl {
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    signal_control_source::SignalControl::with_receiver(receiver)
}

fn failed_signal_wait() -> io::Result<i32> {
    Err(io::Error::other("simulated sigwait failure"))
}

#[test]
fn startup_failures_keep_typed_errors() {
    use signal_control_source::SignalControlError as SourceError;

    assert!(matches!(
        signal_control_source::map_signal_setup_result(Err(io::Error::other("setup failure"))),
        Err(SourceError::OperatingSystem(error)) if error.to_string() == "setup failure"
    ));
    assert!(matches!(
        signal_control_source::map_signal_worker_spawn(Err(io::Error::other("spawn failure"))),
        Err(SourceError::ThreadSpawn(error)) if error.to_string() == "spawn failure"
    ));
    let state = Mutex::new(false);
    let guard = match state.lock() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    assert!(matches!(
        signal_control_source::lock_signal_control_start_state(Err(PoisonError::new(guard))),
        Err(SourceError::StartStatePoisoned)
    ));
}

#[test]
fn control_event_maps_configured_signals_and_rejects_unknown_values() {
    use signal_control_source::{SignalEvent as SourceEvent, control_event};

    assert_eq!(control_event(2), Some(SourceEvent::Interrupt));
    assert_eq!(control_event(15), Some(SourceEvent::TerminationRequested));
    #[cfg(target_os = "linux")]
    assert_eq!(control_event(17), Some(SourceEvent::ChildrenChanged));
    #[cfg(target_os = "macos")]
    assert_eq!(control_event(20), Some(SourceEvent::ChildrenChanged));
    assert_eq!(control_event(-1), None);
}

#[test]
fn worker_failure_stops_siblings_and_receiver() {
    use signal_control_source::{SignalControlError as SourceError, sigwait_loop};

    let worker_failed = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    sigwait_loop(&sender, &worker_failed, failed_signal_wait);
    assert!(worker_failed.load(Ordering::Acquire));
    let mut sibling_wait_calls = 0;
    sigwait_loop(&sender, &worker_failed, || {
        sibling_wait_calls += 1;
        Ok(2)
    });
    assert_eq!(sibling_wait_calls, 0);
    let control = signal_control_source::SignalControl::with_receiver(receiver);
    assert!(matches!(control.receive(), Err(SourceError::SignalWait(_))));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Err(SourceError::WorkerStopped)
    ));
    assert!(matches!(
        control.try_receive(),
        Err(SourceError::WorkerStopped)
    ));
}

#[test]
fn wait_failure_before_return_stops_without_reporting_an_event() {
    use signal_control_source::sigwait_loop;

    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    let mut wait_calls = 0;
    sigwait_loop(&sender, &worker_failed, || {
        wait_calls += 1;
        worker_failed.store(true, Ordering::Release);
        Ok(2)
    });
    assert_eq!(wait_calls, 1);
    assert!(worker_failed.load(Ordering::Acquire));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn unexpected_signal_is_reported_as_worker_failure() {
    use signal_control_source::{SignalControlError as SourceError, sigwait_loop};

    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    sigwait_loop(&sender, &worker_failed, || Ok(-1));
    assert!(worker_failed.load(Ordering::Acquire));
    assert!(matches!(
        receiver.try_recv(),
        Ok(Err(SourceError::UnexpectedSignal(-1)))
    ));
}

#[test]
fn event_send_failure_stops_the_worker() {
    use signal_control_source::sigwait_loop;

    let worker_failed = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    sigwait_loop(&sender, &worker_failed, || Ok(2));
    assert!(!worker_failed.load(Ordering::Acquire));
}

#[test]
fn disconnected_receiver_is_terminal_for_each_receive_method() {
    use signal_control_source::SignalControlError as SourceError;

    assert!(matches!(
        source_disconnected_control().receive(),
        Err(SourceError::WorkerStopped)
    ));
    assert!(matches!(
        source_disconnected_control().receive_timeout(Duration::ZERO),
        Err(SourceError::WorkerStopped)
    ));
    assert!(matches!(
        source_disconnected_control().try_receive(),
        Err(SourceError::WorkerStopped)
    ));
}

#[test]
fn receive_methods_deliver_queued_events_without_marking_failure() {
    use signal_control_source::SignalEvent as SourceEvent;

    let control = source_control_with_result(Ok(SourceEvent::Interrupt));
    assert!(matches!(control.receive(), Ok(SourceEvent::Interrupt)));
    let control = source_control_with_result(Ok(SourceEvent::TerminationRequested));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Ok(Some(SourceEvent::TerminationRequested))
    ));
    let control = source_control_with_result(Ok(SourceEvent::ChildrenChanged));
    assert!(matches!(
        control.try_receive(),
        Ok(Some(SourceEvent::ChildrenChanged))
    ));
}

#[test]
fn receive_methods_return_the_first_worker_error() {
    use signal_control_source::SignalControlError as SourceError;

    let control = source_control_with_result(Err(SourceError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(control.receive(), Err(SourceError::SignalWait(_))));
    let control = source_control_with_result(Err(SourceError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(
        control.receive_timeout(Duration::ZERO),
        Err(SourceError::SignalWait(_))
    ));
    let control = source_control_with_result(Err(SourceError::SignalWait(io::Error::other(
        "wait failure",
    ))));
    assert!(matches!(
        control.try_receive(),
        Err(SourceError::SignalWait(_))
    ));
}

#[test]
fn signal_control_runs_in_a_subprocess() -> Result<(), Box<dyn std::error::Error>> {
    ncl_sys::Thread::block_signal_control_set()?;
    let output = Command::new(std::env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "signal_control_child_receives_events",
            "--nocapture",
        ])
        .env(CHILD_MARKER, "1")
        .output()?;
    assert!(
        output.status.success(),
        "child signal test failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn included_signal_control_source_runs_in_a_subprocess() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(std::env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "included_source_child_starts_signal_control",
            "--nocapture",
        ])
        .env(SOURCE_CHILD_MARKER, "1")
        .output()?;
    assert!(
        output.status.success(),
        "included source signal test failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
#[ignore = "run in a subprocess to isolate process-wide signal setup"]
fn included_source_child_starts_signal_control() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(SOURCE_CHILD_MARKER).is_none() {
        return Ok(());
    }
    let control = signal_control_source::SignalControl::start()?;
    signal_control_source::SignalControl::prepare_current_thread()?;
    assert!(format!("{control:?}").starts_with("SignalControl"));
    assert!(matches!(
        signal_control_source::SignalControl::start(),
        Err(signal_control_source::SignalControlError::AlreadyStarted)
    ));
    Ok(())
}

#[test]
#[ignore = "run in a subprocess after the parent blocks the test harness signal mask"]
fn signal_control_child_receives_events() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return Ok(());
    }
    let control = SignalControl::start()?;
    SignalControl::prepare_current_thread()?;
    assert!(format!("{control:?}").starts_with("SignalControl"));
    assert_eq!(control.try_receive()?, None);
    assert_eq!(control.receive_timeout(Duration::from_millis(1))?, None);
    assert!(matches!(
        SignalControl::start(),
        Err(SignalControlError::AlreadyStarted)
    ));
    for (name, expected) in [
        ("INT", SignalEvent::Interrupt),
        ("TERM", SignalEvent::TerminationRequested),
    ] {
        send_process_signal(name)?;
        receive_process_and_child_events(&control, expected)?;
    }
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--list")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    receive_expected_event(&control, SignalEvent::ChildrenChanged)?;
    assert!(child.wait()?.success());
    Ok(())
}

fn send_process_signal(name: &str) -> io::Result<()> {
    let process_id = std::process::id().to_string();
    let status = Command::new("kill")
        .args([format!("-{name}"), process_id])
        .status()?;
    assert!(status.success(), "kill utility should send {name}");
    Ok(())
}

fn receive_process_and_child_events(
    control: &SignalControl,
    expected: SignalEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut received_process_event = false;
    let mut received_child_event = false;
    while !received_process_event || !received_child_event {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        if timeout.is_zero() {
            return Err(
                io::Error::new(io::ErrorKind::TimedOut, "signal events were not received").into(),
            );
        }
        match control.receive_timeout(timeout)? {
            Some(event) if event == expected => received_process_event = true,
            Some(SignalEvent::ChildrenChanged) => received_child_event = true,
            Some(event) => {
                return Err(io::Error::other(format!("unexpected signal event {event:?}")).into());
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "signal events were not received",
                )
                .into());
            }
        }
    }
    Ok(())
}

fn receive_expected_event(
    control: &SignalControl,
    expected: SignalEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    for _ in 0..16 {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        if timeout.is_zero() {
            break;
        }
        let Some(event) = control.receive_timeout(timeout)? else {
            return Err(
                io::Error::new(io::ErrorKind::TimedOut, "signal event was not received").into(),
            );
        };
        if event == expected {
            return Ok(());
        }
        if event != SignalEvent::ChildrenChanged {
            return Err(io::Error::other(format!("unexpected signal event {event:?}")).into());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("signal event {expected:?} was not received"),
    )
    .into())
}

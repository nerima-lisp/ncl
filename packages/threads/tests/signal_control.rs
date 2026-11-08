//! Integration tests for the runtime signal event queue.

use ncl_threads::signal_control::SignalControlError;
use ncl_threads::signal_control::{SignalControl, SignalEvent};
use std::error::Error as _;
use std::io;
use std::process::{Command, Stdio};
use std::time::Duration;

const CHILD_MARKER: &str = "NCL_SIGWAIT_CHILD";

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

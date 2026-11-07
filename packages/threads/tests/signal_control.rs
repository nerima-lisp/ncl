//! Integration tests for the runtime signal event queue.

use ncl_threads::signal_control::{SignalControl, SignalEvent};
use std::process::Command;
use std::time::Duration;

const CHILD_MARKER: &str = "NCL_SIGWAIT_CHILD";

#[test]
fn signal_control_runs_in_a_subprocess() {
    ncl_sys::Thread::block_signal_control_set()
        .expect("the child process must inherit the blocked signal set");
    let executable = std::env::current_exe().expect("test executable path should be available");
    let output = Command::new(executable)
        .args([
            "--ignored",
            "--exact",
            "signal_control_child_receives_events",
            "--nocapture",
        ])
        .env(CHILD_MARKER, "1")
        .output()
        .expect("child test process should start");
    assert!(
        output.status.success(),
        "child signal test failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "run in a subprocess after the parent blocks the test harness signal mask"]
fn signal_control_child_receives_events() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }
    let control = SignalControl::start().expect("sigwait control thread should start");
    for (name, expected) in [
        ("INT", SignalEvent::Interrupt),
        ("TERM", SignalEvent::TerminationRequested),
    ] {
        send_process_signal(name);
        receive_process_and_child_events(&control, expected);
    }
    let mut child =
        Command::new(std::env::current_exe().expect("test executable path should exist"))
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("child process should start");
    receive_expected_event(&control, SignalEvent::ChildrenChanged);
    assert!(child.wait().expect("child process should exit").success());
}

fn send_process_signal(name: &str) {
    let process_id = std::process::id().to_string();
    let status = Command::new("kill")
        .args([format!("-{name}"), process_id])
        .status()
        .expect("kill utility should start");
    assert!(status.success(), "kill utility should send {name}");
}

fn receive_process_and_child_events(control: &SignalControl, expected: SignalEvent) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut received_process_event = false;
    let mut received_child_event = false;
    while !received_process_event || !received_child_event {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!timeout.is_zero(), "signal events were not received");
        match control
            .receive_timeout(timeout)
            .expect("sigwait worker should remain active")
        {
            Some(event) if event == expected => received_process_event = true,
            Some(SignalEvent::ChildrenChanged) => received_child_event = true,
            Some(event) => panic!("unexpected signal event {event:?}"),
            None => panic!("signal events were not received"),
        }
    }
}

fn receive_expected_event(control: &SignalControl, expected: SignalEvent) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    for _ in 0..16 {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        if timeout.is_zero() {
            break;
        }
        let event = control
            .receive_timeout(timeout)
            .expect("sigwait worker should remain active");
        if event == Some(expected) {
            return;
        }
        assert_eq!(
            event,
            Some(SignalEvent::ChildrenChanged),
            "only coalesced child-status events may precede {expected:?}"
        );
    }
    panic!("sigwait event {expected:?} was not received");
}

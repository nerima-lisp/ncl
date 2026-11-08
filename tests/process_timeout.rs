//! Distinguish child CPU-time and wall-clock timeout failures.

#[path = "common/mod.rs"]
mod common;

use common::{ChildRunError, ChildTimeouts, run_command_with_limits, run_ncl_with_limits};
use std::process::Command;
use std::time::Duration;

#[test]
fn nonterminating_lisp_form_hits_the_cpu_time_limit() {
    let result = run_ncl_with_limits(
        "(loop while t do (+ 1 1))",
        ChildTimeouts::new(2, Duration::from_secs(120)),
    );
    assert!(
        matches!(result, Err(ChildRunError::CpuTimeLimitExceeded { .. })),
        "expected CPU limit failure, got {result:?}"
    );
}

#[test]
fn sleeping_child_hits_the_wall_clock_limit() {
    let executable = std::env::current_exe()
        .unwrap_or_else(|error| panic!("failed to locate test executable: {error}"));
    let mut command = Command::new(executable);
    command.args(["--exact", "sleeping_child", "--ignored", "--nocapture"]);
    let result = run_command_with_limits(
        &mut command,
        None,
        ChildTimeouts::new(30, Duration::from_secs(5)),
    );
    match result {
        Err(ChildRunError::WallClockLimitExceeded { stdout, .. }) => assert!(
            String::from_utf8_lossy(&stdout).contains("SLEEP_CHILD_READY"),
            "sleeping child did not report readiness before wall timeout: {}",
            String::from_utf8_lossy(&stdout)
        ),
        result => panic!("expected wall-clock limit failure, got {result:?}"),
    }
}

#[test]
#[ignore = "re-entered by sleeping_child_hits_the_wall_clock_limit"]
fn sleeping_child() {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(error) = writeln!(stdout, "SLEEP_CHILD_READY") {
        panic!("failed to write sleeper readiness: {error}");
    }
    if let Err(error) = stdout.flush() {
        panic!("failed to flush sleeper readiness: {error}");
    }
    std::thread::sleep(Duration::from_secs(60));
}

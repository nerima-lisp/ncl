//! Process-isolated checks for runtime pass allocation diagnostics.

use ncl_runtime::{AllocationTrackingAllocator, OptimizationMode, Runtime, RuntimeOptions};
use ncl_sys::AllocationTrackingError;
use std::process::Command;

#[global_allocator]
static ALLOCATOR: AllocationTrackingAllocator = AllocationTrackingAllocator;

const SOURCE: &str = "(+ 20 22)";
const TRACKED_PHASE_BEGIN: &str = "TRACKED_PHASE_BEGIN";

#[test]
fn pass_allocation_diagnostics_report_bytes_without_changing_results()
-> Result<(), Box<dyn std::error::Error>> {
    ALLOCATOR.activate();
    let options = RuntimeOptions::default().with_pass_allocation_diagnostics(true);
    let mut tracked = Runtime::with_options(options)?;
    let value = tracked.eval(SOURCE)?;
    assert_eq!(tracked.format_result(value), "42");

    let output = Command::new(std::env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "allocation_diagnostics_child_reports_pass_bytes",
            "--nocapture",
        ])
        .output()?;
    assert!(
        output.status.success(),
        "child stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.lines().any(|line| line == "OBSERVER_RESULT=42"));
    assert!(stdout.lines().any(|line| line == "TRACKED_RESULT=42"));

    let stderr = String::from_utf8_lossy(&output.stderr);
    let (observer_events, tracked_events) = stderr
        .split_once(TRACKED_PHASE_BEGIN)
        .ok_or_else(|| std::io::Error::other("tracked phase marker is missing"))?;
    assert!(
        observer_events.lines().any(
            |line| line.starts_with("ncl-opt iteration=") && !line.contains("allocated_bytes=")
        )
    );
    let tracked_event = tracked_events
        .lines()
        .find(|line| line.starts_with("ncl-opt iteration=") && line.contains("allocated_bytes="))
        .ok_or_else(|| {
            std::io::Error::other("allocation bytes are missing from optimizer diagnostics")
        })?;
    let allocated_bytes = tracked_event
        .split_whitespace()
        .find_map(|field| field.strip_prefix("allocated_bytes="))
        .and_then(|value| value.parse::<u128>().ok());
    assert!(allocated_bytes.is_some(), "{tracked_event}");

    assert_eq!(
        AllocationTrackingError::ThreadStateUnavailable.to_string(),
        "allocation tracking state is unavailable"
    );
    Ok(())
}

#[test]
#[ignore = "invoked by the parent test to capture optimizer diagnostics"]
fn allocation_diagnostics_child_reports_pass_bytes() -> Result<(), Box<dyn std::error::Error>> {
    ALLOCATOR.activate();

    let mut observer = Runtime::with_options(RuntimeOptions::new(OptimizationMode::Default, true))?;
    let value = observer.eval(SOURCE)?;
    println!("OBSERVER_RESULT={}", observer.format_result(value));

    eprintln!("{TRACKED_PHASE_BEGIN}");
    let options = RuntimeOptions::default().with_pass_allocation_diagnostics(true);
    let mut tracked = Runtime::with_options(options)?;
    let value = tracked.eval(SOURCE)?;
    println!("TRACKED_RESULT={}", tracked.format_result(value));
    Ok(())
}

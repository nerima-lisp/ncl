//! Integration tests for code arena allocation and retirement.

use std::sync::Mutex;

use ncl_sys::{CodeError, alloc_code};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn code_extents_are_page_rounded_in_order_and_reused() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let page = system_page_size();

    assert!(matches!(alloc_code(0), Err(CodeError::EmptyAllocation)));
    assert!(matches!(
        alloc_code(usize::MAX),
        Err(CodeError::OutOfBounds)
    ));

    let first = test_value(alloc_code(page - 1), "first code extent allocation failed");
    let second = test_value(alloc_code(1), "second code extent allocation failed");
    let third = test_value(alloc_code(page + 1), "third code extent allocation failed");
    let first_address = first.address();
    let second_address = second.address();
    let third_address = third.address();

    assert_eq!(first_address % page, 0);
    assert_eq!(second_address % page, 0);
    assert_eq!(third_address % page, 0);
    assert_eq!(second_address, first_address + page);
    assert_eq!(third_address, second_address + page);
    assert!(first_address + page <= second_address);
    assert!(second_address + page <= third_address);
    #[cfg(target_os = "linux")]
    assert!(proc_map_contains(first_address));

    drop(third);
    drop(second);
    drop(first);
    #[cfg(target_os = "linux")]
    assert!(!proc_map_contains(first_address));

    let reused = test_value(alloc_code(1), "allocation after arena retirement failed");
    assert_eq!(reused.address() % page, 0);
    #[cfg(target_os = "macos")]
    assert_eq!(reused.address(), first_address);
    #[cfg(target_os = "linux")]
    assert!(proc_map_contains(reused.address()));

    #[cfg(target_os = "macos")]
    assert!(matches!(
        alloc_code((1usize << 30) + page),
        Err(CodeError::CapacityExceeded)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_hundred_thousand_live_extents_fit_resource_budget() {
    const CHILD_RUN: &str = "NCL_CODE_ARENA_STRESS_CHILD";
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if std::env::var_os(CHILD_RUN).is_some() {
        run_linux_stress_child();
        return;
    }

    let executable = std::env::current_exe()
        .unwrap_or_else(|error| panic!("test executable path unavailable: {error}"));
    for run in 1..=5 {
        let output = std::process::Command::new(&executable)
            .args([
                "--exact",
                "linux_hundred_thousand_live_extents_fit_resource_budget",
                "--nocapture",
            ])
            .env(CHILD_RUN, "1")
            .output()
            .unwrap_or_else(|error| panic!("could not start stress process {run}: {error}"));
        assert!(
            output.status.success(),
            "stress run {run} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let Some(metric) = stdout
            .lines()
            .find(|line| line.starts_with("CODE_ARENA_RESULT "))
        else {
            panic!("stress child did not report RSS and map metrics: {stdout}");
        };
        println!("run {run}: {metric}");
    }
}

#[cfg(target_os = "linux")]
fn run_linux_stress_child() {
    const CODE_COUNT: usize = 100_000;
    const ARENA_SIZE: usize = 16 * 1024 * 1024;

    let page = system_page_size();
    let mut code = Vec::with_capacity(CODE_COUNT);
    let baseline_rss_kib = proc_status_kib("VmRSS");
    let maps_before = proc_map_count();
    for index in 0..CODE_COUNT {
        let mut extent =
            alloc_code(1).unwrap_or_else(|error| panic!("allocation {index} failed: {error:?}"));
        extent
            .write_code(0, &[0])
            .unwrap_or_else(|error| panic!("write {index} failed: {error:?}"));
        ncl_sys::publish_code(&mut extent)
            .unwrap_or_else(|error| panic!("publish {index} failed: {error:?}"));
        code.push(extent);
    }

    let peak_rss_kib = proc_status_kib("VmHWM");
    let maps_after = proc_map_count();
    let rss_increase_kib = peak_rss_kib.saturating_sub(baseline_rss_kib);
    let mapped_code_bytes = CODE_COUNT
        .checked_mul(page)
        .unwrap_or_else(|| panic!("stress code size overflowed"));
    let arena_count = mapped_code_bytes.div_ceil(ARENA_SIZE);
    let maximum_map_increase = arena_count * 3 + 32;
    let map_increase = maps_after.saturating_sub(maps_before);

    assert!(
        rss_increase_kib <= 512 * 1024,
        "100,000 extents increased RSS by {rss_increase_kib} KiB"
    );
    assert!(
        map_increase <= maximum_map_increase,
        "100,000 extents added {map_increase} VM maps; arena bound is {maximum_map_increase}"
    );
    println!(
        "CODE_ARENA_RESULT count={CODE_COUNT} rss_baseline_kib={baseline_rss_kib} rss_peak_kib={peak_rss_kib} rss_increase_kib={rss_increase_kib} maps_before={maps_before} maps_after={maps_after} arena_count={arena_count}"
    );
    drop(code);
}

fn system_page_size() -> usize {
    let output = std::process::Command::new("getconf")
        .arg("PAGESIZE")
        .output()
        .unwrap_or_else(|error| panic!("getconf is required for page-size checks: {error}"));
    assert!(output.status.success(), "getconf PAGESIZE failed");
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("getconf output was not UTF-8: {error}"))
        .trim()
        .parse()
        .unwrap_or_else(|error| panic!("getconf returned an invalid page size: {error}"))
}

#[cfg(target_os = "linux")]
fn proc_status_kib(field: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status")
        .unwrap_or_else(|error| panic!("could not read process RSS status: {error}"));
    status
        .lines()
        .find(|line| line.starts_with(&format!("{field}:")))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("{field} is absent from /proc/self/status"))
}

#[cfg(target_os = "linux")]
fn proc_map_count() -> usize {
    std::fs::read_to_string("/proc/self/maps")
        .unwrap_or_else(|error| panic!("could not read process VM maps: {error}"))
        .lines()
        .count()
}

#[cfg(target_os = "linux")]
fn proc_map_contains(address: usize) -> bool {
    let maps = std::fs::read_to_string("/proc/self/maps")
        .unwrap_or_else(|error| panic!("could not read process VM maps: {error}"));
    maps.lines().any(|line| {
        let Some(range) = line.split_whitespace().next() else {
            return false;
        };
        let Some((start, end)) = range.split_once('-') else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            usize::from_str_radix(start, 16),
            usize::from_str_radix(end, 16),
        ) else {
            return false;
        };
        (start..end).contains(&address)
    })
}

fn test_value<T>(result: Result<T, CodeError>, message: &str) -> T {
    result.unwrap_or_else(|error| panic!("{message}: {error:?}"))
}

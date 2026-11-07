#![allow(missing_docs)]

use std::process::{Command, Output};

fn profiler() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl-profiler"))
}

fn output(command: &mut Command) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl-profiler: {error}"))
}

#[test]
fn help_and_version_describe_the_instrumented_runner() {
    let help = output(profiler().arg("--help"));
    assert!(help.status.success(), "{help:?}");
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("Usage: ncl-profiler"));
    assert!(help.contains("allocated_bytes"));
    assert!(help.contains("ordinary compile-performance measurements"));

    let version = output(profiler().arg("--version"));
    assert!(version.status.success(), "{version:?}");
    assert!(String::from_utf8_lossy(&version.stdout).contains("ncl-profiler"));
    assert!(String::from_utf8_lossy(&version.stdout).contains("(instrumented)"));
}

#[test]
fn eval_with_diagnostics_returns_a_value_and_per_pass_metrics() {
    let result = output(profiler().args(["--opt-diagnostics", "--eval", "(+ 1 2)"]));
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), "3\n");

    let stderr = String::from_utf8_lossy(&result.stderr);
    let pass_lines = stderr
        .lines()
        .filter(|line| line.starts_with("ncl-opt "))
        .collect::<Vec<_>>();
    assert!(!pass_lines.is_empty(), "{stderr}");
    assert!(
        pass_lines
            .iter()
            .all(|line| { line.contains("elapsed_ns=") && line.contains("allocated_bytes=") })
    );
}

#[test]
fn script_mode_executes_the_file_without_printing_its_result() {
    let path = std::env::temp_dir().join(format!("ncl-profiler-cli-{}.lisp", std::process::id()));
    std::fs::write(
        &path,
        "(progn (write-string \"profiler-script-ran\") (terpri))",
    )
    .unwrap_or_else(|error| panic!("script creation failed: {error}"));
    let path = path.to_string_lossy().into_owned();

    let result = output(profiler().args(["--script", &path]));
    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("script cleanup failed: {error}"));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout),
        "profiler-script-ran\n"
    );
}

#[test]
fn unknown_options_are_rejected() {
    let result = output(profiler().arg("--unsupported-option"));
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported option"));
}

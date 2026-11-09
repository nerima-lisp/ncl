#![allow(missing_docs)]

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

// CLHS Function +, section 12.1.1, defines the result of this numeric sum.
const ADDITION_OUTPUT: &str = "3\n";

fn profiler() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl-profiler"))
}

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn output(command: &mut Command) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl-profiler: {error}"))
}

fn ncl_output(command: &mut Command) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl: {error}"))
}

#[test]
fn ncl_version_flag_prints_the_current_package_version() {
    let result = ncl_output(ncl().arg("--version"));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout),
        format!("ncl {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(result.stderr.is_empty());
}

#[test]
fn ncl_opt_diagnostics_report_timings_without_allocator_metrics() {
    let result = ncl_output(ncl().args(["--opt-diagnostics", "--eval", "(+ 1 2)"]));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);

    let stderr = String::from_utf8_lossy(&result.stderr);
    let pass_lines = stderr
        .lines()
        .filter(|line| line.starts_with("ncl-opt "))
        .collect::<Vec<_>>();
    assert!(!pass_lines.is_empty(), "{stderr}");
    assert!(pass_lines.iter().all(|line| line.contains("elapsed_ns=")));
    assert!(!stderr.contains("allocated_bytes="), "{stderr}");
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
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);

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
fn valid_optimization_modes_preserve_the_eval_result() {
    for mode in ["default", "off"] {
        let option = format!("--optimization={mode}");
        let result = output(profiler().args([option.as_str(), "--eval", "(+ 1 2)"]));

        assert!(result.status.success(), "mode={mode}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);
        assert!(result.stderr.is_empty(), "mode={mode}: {:?}", result.stderr);
    }
}

#[test]
fn optimization_off_with_diagnostics_keeps_the_result_without_pass_events() {
    let result = output(profiler().args([
        "--opt-diagnostics",
        "--optimization=off",
        "--eval",
        "(+ 1 2)",
    ]));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);
    assert!(result.stderr.is_empty());
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
fn invalid_optimization_mode_reports_the_mode_error() {
    let result = output(profiler().args(["--optimization=fast", "--eval", "(+ 1 2)"]));

    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "ncl-profiler: unsupported optimization mode \"fast\"\n"
    );
}

#[test]
fn eval_without_source_reports_the_missing_argument() {
    let result = output(profiler().arg("--eval"));

    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "--eval requires a source string\n"
    );
}

#[test]
fn eval_with_extra_source_reports_the_argument_limit() {
    let result = output(profiler().args(["--eval", "(+ 1 2)", "(+ 3 4)"]));

    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "--eval accepts exactly one source string\n"
    );
}

#[test]
fn file_modes_report_a_missing_path_before_opening_a_file() {
    for mode in ["--load", "--script", "--compile-file"] {
        let result = output(profiler().arg(mode));

        assert_eq!(result.status.code(), Some(2), "{mode}: {result:?}");
        assert!(result.stdout.is_empty(), "{mode}: {:?}", result.stdout);
        assert_eq!(
            String::from_utf8_lossy(&result.stderr),
            format!("{mode} requires a file path\n")
        );
    }
}

#[test]
fn file_modes_reject_an_extra_path_before_opening_the_first_file() {
    for mode in ["--load", "--script", "--compile-file"] {
        let result = output(profiler().args([mode, "missing.lisp", "extra.lisp"]));

        assert_eq!(result.status.code(), Some(2), "{mode}: {result:?}");
        assert!(result.stdout.is_empty(), "{mode}: {:?}", result.stdout);
        assert_eq!(
            String::from_utf8_lossy(&result.stderr),
            format!("{mode} accepts exactly one file path\n")
        );
    }
}

#[test]
fn load_mode_prints_the_loaded_form_result() {
    let path = std::env::temp_dir().join(format!("ncl-profiler-load-{}.lisp", std::process::id()));
    std::fs::write(&path, "(+ 1 2)")
        .unwrap_or_else(|error| panic!("source file creation failed: {error}"));
    let path = path.to_string_lossy().into_owned();

    let result = output(profiler().args(["--load", &path]));
    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);
    assert!(result.stderr.is_empty());
}

#[test]
fn compile_file_mode_prints_the_compiled_form_result() {
    let path = std::env::temp_dir().join(format!(
        "ncl-profiler-compile-file-{}.lisp",
        std::process::id()
    ));
    std::fs::write(&path, "(+ 1 2)")
        .unwrap_or_else(|error| panic!("source file creation failed: {error}"));
    let path = path.to_string_lossy().into_owned();
    let fasl = std::path::Path::new(&path).with_extension("fasl");

    let result = output(profiler().args(["--compile-file", &path]));
    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("source cleanup failed: {error}"));
    if fasl.exists() {
        std::fs::remove_file(&fasl).unwrap_or_else(|error| panic!("FASL cleanup failed: {error}"));
    }

    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);
    assert!(result.stderr.is_empty());
}

#[test]
fn repl_evaluates_a_complete_multiline_form_before_eof() {
    let mut command = profiler();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start ncl-profiler REPL: {error}"));
    let Some(mut stdin) = child.stdin.take() else {
        panic!("REPL stdin is piped");
    };
    stdin
        .write_all(b"(+ 1\n2)\n")
        .unwrap_or_else(|error| panic!("failed to write REPL input: {error}"));
    drop(stdin);

    let result = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to collect REPL output: {error}"));

    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), ADDITION_OUTPUT);
    assert_eq!(String::from_utf8_lossy(&result.stderr), "> > > ");
}

#[test]
fn repl_reports_a_runtime_error_and_continues_to_the_next_prompt() {
    let mut command = profiler();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start ncl-profiler REPL: {error}"));
    let Some(mut stdin) = child.stdin.take() else {
        panic!("REPL stdin is piped");
    };
    stdin
        .write_all(b")\n")
        .unwrap_or_else(|error| panic!("failed to write REPL input: {error}"));
    drop(stdin);

    let result = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to collect REPL output: {error}"));

    assert!(result.status.success(), "{result:?}");
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.starts_with("> ncl-profiler: "), "{stderr}");
    assert!(stderr.ends_with("> "), "{stderr}");
}

#[cfg(unix)]
#[test]
fn repl_reports_stdin_read_failure_and_exits() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::File::open("/")?;
    let result = profiler()
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.starts_with("> ncl-profiler: "), "{stderr}");
    assert!(!stderr.ends_with("> "), "{stderr}");
    Ok(())
}

#[test]
fn invalid_eval_reports_a_runtime_error_without_stdout() {
    let result = output(profiler().args(["--eval", ")"]));

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.starts_with("ncl-profiler: "), "{stderr}");
    assert!(stderr.len() > "ncl-profiler: \n".len(), "{stderr}");
}

#[test]
fn load_missing_file_reports_a_runtime_error_without_stdout() {
    let path = std::env::temp_dir()
        .join(format!("ncl-profiler-missing-{}.lisp", std::process::id()))
        .with_extension("absent.lisp");
    let path = path.to_string_lossy().into_owned();
    let result = output(profiler().args(["--load", &path]));

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.starts_with("ncl-profiler: "), "{stderr}");
    assert!(stderr.len() > "ncl-profiler: \n".len(), "{stderr}");
}

#[test]
fn unknown_options_are_rejected() {
    let result = output(profiler().arg("--unsupported-option"));
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported option"));
}

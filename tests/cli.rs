//! End-to-end tests for the command-line interface.

use std::fs;
use std::io::Write;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(10);

fn run(arguments: &[&str]) -> Output {
    let child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("ncl binary should be executable: {error}"));
    wait_for_process(child, arguments)
}

fn run_with_stdin(arguments: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("ncl binary should be executable: {error}"));
    child
        .stdin
        .take()
        .unwrap_or_else(|| panic!("stdin pipe should be available"))
        .write_all(input.as_bytes())
        .unwrap_or_else(|error| panic!("REPL input should be writable: {error}"));
    wait_for_process(child, arguments)
}

fn wait_for_process(mut child: Child, arguments: &[&str]) -> Output {
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .unwrap_or_else(|error| panic!("ncl process status should be readable: {error}"))
            .is_some()
        {
            return child
                .wait_with_output()
                .unwrap_or_else(|error| panic!("ncl process output should be readable: {error}"));
        }
        if started.elapsed() >= PROCESS_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("ncl process timed out after {PROCESS_TIMEOUT:?}: {arguments:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn compiled_mode_runs_definitions_across_eval_arguments() {
    let output = run(&[
        "--compiled",
        "--eval",
        "(define answer 1)",
        "--eval",
        "(setq answer (+ answer 2))",
        "--eval",
        "answer",
    ]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n3\n3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn interpreter_mode_expands_user_macros() {
    let output = run(&[
        "--eval",
        r"(progn (defmacro twice (x) `(+ ,x ,x)) (twice 4))",
    ]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "8\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn compiled_mode_expands_user_macros() {
    let output = run(&[
        "--compiled",
        "--eval",
        r"(progn (defmacro twice (x) `(+ ,x ,x)) (twice 4))",
    ]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "8\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn missing_file_is_a_runtime_error() {
    let output = run(&["--file", "/definitely-not-present-ncl-test-file"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot read"));
}

#[test]
fn unknown_option_is_a_usage_error() {
    let output = run(&["--not-an-option"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown option"));
}

#[test]
fn help_and_version_are_successful() {
    for argument in ["--help", "-h", "--version", "-V"] {
        let output = run(&[argument]);

        assert!(output.status.success(), "{argument} should succeed");
        assert!(!output.stdout.is_empty(), "{argument} should print output");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn short_options_and_quiet_mode_work() {
    let output = run(&["-e", "(+ 1 2)", "-q"]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn file_mode_evaluates_source() {
    let path = std::env::temp_dir().join(format!(
        "ncl-cli-test-{}-{}.lisp",
        std::process::id(),
        "file-mode"
    ));
    if let Err(error) = fs::write(&path, "(+ 20 22)\n") {
        panic!("test file should be writable: {error}");
    }

    let Some(path_string) = path.to_str() else {
        panic!("test path should be UTF-8");
    };
    let output = run(&["-f", path_string]);
    let _ = fs::remove_file(&path);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn compiled_file_mode_evaluates_source() {
    let path = std::env::temp_dir().join(format!(
        "ncl-cli-test-{}-compiled-file-mode.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&path, "(+ 20 22)\n") {
        panic!("test file should be writable: {error}");
    }

    let Some(path_string) = path.to_str() else {
        panic!("test path should be UTF-8");
    };
    let output = run(&["--compiled", "--file", path_string]);
    let _ = fs::remove_file(&path);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_arguments_and_runtime_errors_are_reported() {
    let cases = [
        (&["--eval"][..], "--eval requires a source string"),
        (&["--file"][..], "--file requires a path"),
        (&["input.lisp"][..], "unexpected argument input.lisp"),
    ];
    for (arguments, message) in cases {
        let output = run(arguments);

        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }

    let output = run(&["--eval", "("]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());

    let output = run(&["--compiled", "--eval", "("]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
}

#[test]
fn invalid_utf8_file_is_reported_as_an_io_error() {
    let path = std::env::temp_dir().join(format!(
        "ncl-cli-test-{}-invalid-utf8.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&path, [0xff, 0xfe, 0xfd]) {
        panic!("test file should be writable: {error}");
    }

    let Some(path_string) = path.to_str() else {
        panic!("test path should be UTF-8");
    };
    let output = run(&["--file", path_string]);
    let _ = fs::remove_file(&path);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot read"));
}

#[test]
fn repl_evaluates_input_and_reports_errors() {
    let output = run_with_stdin(&["--repl"], "\n(+ 2 3)\n(\n");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ncl> ncl> 5\nncl> ncl> "
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn compiled_quiet_repl_suppresses_prompts_and_values() {
    let output = run_with_stdin(&["--repl", "--compiled", "--quiet"], "(+ 2 3)\n");

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn compiled_repl_prints_values_and_reports_errors() {
    let output = run_with_stdin(&["--repl", "--compiled"], "(+ 2 3)\n(\n");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ncl> 5\nncl> ncl> "
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn no_arguments_start_repl_and_exit_on_eof() {
    let output = run(&[]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ncl> ");
    assert!(output.stderr.is_empty());
}

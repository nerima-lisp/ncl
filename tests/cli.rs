use std::process::Command;

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(arguments)
        .output()
        .expect("ncl binary should be executable")
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
        r#"(progn (defmacro twice (x) `(+ ,x ,x)) (twice 4))"#,
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
        r#"(progn (defmacro twice (x) `(+ ,x ,x)) (twice 4))"#,
    ]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "8\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn compile_mode_reports_artifact_without_executing_it() {
    let output = run(&[
        "--compile",
        "--eval",
        "(define answer 42)",
        "--eval",
        "answer",
    ]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 2);
    assert!(stdout.lines().all(|line| line.starts_with("compiled ")));
    assert!(output.stderr.is_empty());
}

#[test]
fn compile_mode_expands_macros_before_reporting() {
    let output = run(&[
        "--compile",
        "--eval",
        r#"(defmacro twice (x) `(+ ,x ,x))"#,
        "--eval",
        "(twice 4)",
    ]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 2);
    assert!(output.stderr.is_empty());
}

#[test]
fn compile_mode_requires_an_input_source() {
    let output = run(&["--compile"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --eval or --file"));
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

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
fn help_and_version_are_stable_successful_commands() {
    let help = run(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
    assert!(help.stderr.is_empty());

    let version = run(&["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("ncl "));
    assert!(version.stderr.is_empty());
}

#[test]
fn file_and_quiet_modes_select_the_requested_output_policy() {
    let path = std::env::temp_dir().join(format!("ncl-cli-{}.lisp", std::process::id()));
    std::fs::write(&path, "(+ 2 3)").expect("temporary source should be writable");

    let file = run(&["--file", path.to_str().expect("temporary path is valid")]);
    assert!(file.status.success());
    assert_eq!(String::from_utf8_lossy(&file.stdout), "5\n");
    assert!(file.stderr.is_empty());

    let quiet = run(&["--quiet", "--eval", "(+ 2 3)"]);
    assert!(quiet.status.success());
    assert!(quiet.stdout.is_empty());
    assert!(quiet.stderr.is_empty());

    std::fs::remove_file(path).expect("temporary source should be removable");
}

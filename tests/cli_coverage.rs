#![allow(missing_docs)]

use std::process::{Command, Output};

fn ncl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
}

fn run(args: &[&str]) -> Output {
    ncl()
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl: {error}"))
}

#[test]
fn version_options_print_the_package_version() {
    for option in ["--version", "-V"] {
        let result = run(&[option]);
        assert_eq!(result.status.code(), Some(0), "{option}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), "ncl 0.1.0\n");
        assert!(result.stderr.is_empty(), "{option}: {result:?}");
    }
}

#[test]
fn cli_rejects_missing_or_extra_arguments_with_usage_errors() {
    for args in [
        vec!["--eval"],
        vec!["--load"],
        vec!["--script"],
        vec!["--compile-file"],
        vec!["--eval", "1", "extra"],
        vec!["--load", "file.lisp", "extra"],
        vec!["--script", "file.lisp", "extra"],
        vec!["--compile-file", "file.lisp", "extra"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {result:?}");
        assert!(!result.stdout.is_empty() || result.stderr.starts_with(&b"--"[..]));
    }
}

#[test]
fn cli_rejects_unknown_options() {
    let result = run(&["--not-an-option"]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&result.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "unsupported option: --not-an-option\n"
    );
}

#[test]
fn cli_rejects_positional_arguments() {
    let result = run(&["program.lisp"]);
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "unsupported option: program.lisp\n"
    );
}

#[test]
fn cli_reports_evaluation_and_file_errors() {
    let eval = run(&["--eval", "("]);
    assert_eq!(eval.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&eval.stdout), "");
    assert!(String::from_utf8_lossy(&eval.stderr).starts_with("ncl: "));

    for mode in ["--load", "--script", "--compile-file"] {
        let result = run(&[mode, "/tmp/ncl-file-that-does-not-exist.lisp"]);
        assert_eq!(result.status.code(), Some(1), "{mode}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), "", "{mode}");
        assert!(
            String::from_utf8_lossy(&result.stderr).starts_with("ncl: "),
            "{mode}: {result:?}"
        );
    }
}

#[test]
fn cli_reports_source_errors_for_existing_files() {
    let path = std::env::temp_dir().join(format!(
        "ncl-cli-invalid-source-{}.lisp",
        std::process::id()
    ));
    std::fs::write(&path, "(")
        .unwrap_or_else(|error| panic!("failed to create invalid source: {error}"));
    let path = path.to_string_lossy().into_owned();

    for mode in ["--load", "--script", "--compile-file"] {
        let result = run(&[mode, &path]);
        assert_eq!(result.status.code(), Some(1), "{mode}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stdout), "", "{mode}");
        assert!(
            String::from_utf8_lossy(&result.stderr).starts_with("ncl: "),
            "{mode}: {result:?}"
        );
    }

    std::fs::remove_file(&path)
        .unwrap_or_else(|error| panic!("failed to remove invalid source: {error}"));
}

#[test]
fn cli_file_modes_report_success_and_script_suppresses_values() {
    let path =
        std::env::temp_dir().join(format!("ncl-cli-mode-success-{}.lisp", std::process::id()));
    std::fs::write(&path, "41").unwrap_or_else(|error| panic!("failed to create source: {error}"));
    let path = path.to_string_lossy().into_owned();

    let load = run(&["--load", &path]);
    assert_eq!(load.status.code(), Some(0), "{load:?}");
    assert_eq!(String::from_utf8_lossy(&load.stdout), "41\n");
    assert!(load.stderr.is_empty(), "{load:?}");

    let script = run(&["--script", &path]);
    assert_eq!(script.status.code(), Some(0), "{script:?}");
    assert!(script.stdout.is_empty(), "{script:?}");
    assert!(script.stderr.is_empty(), "{script:?}");

    let compile = run(&["--compile-file", &path]);
    assert_eq!(compile.status.code(), Some(0), "{compile:?}");
    assert_eq!(String::from_utf8_lossy(&compile.stdout), "41\n");
    assert!(compile.stderr.is_empty(), "{compile:?}");

    let fasl = std::path::Path::new(&path).with_extension("fasl");
    assert!(fasl.is_file(), "{fasl:?}");
    std::fs::remove_file(&path).unwrap_or_else(|error| panic!("failed to remove source: {error}"));
    std::fs::remove_file(&fasl).unwrap_or_else(|error| panic!("failed to remove fasl: {error}"));
}

#[test]
fn repl_exits_cleanly_after_incomplete_form_at_eof() {
    let mut child = ncl()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start ncl REPL: {error}"));
    let mut stdin = child
        .stdin
        .take()
        .unwrap_or_else(|| panic!("REPL stdin unavailable"));
    std::io::Write::write_all(&mut stdin, b"(\n")
        .unwrap_or_else(|error| panic!("failed to write REPL input: {error}"));
    drop(stdin);

    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to collect REPL output: {error}"));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).is_empty(),
        "{output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("> "),
        "{output:?}"
    );
}

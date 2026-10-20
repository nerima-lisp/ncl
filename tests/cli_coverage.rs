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

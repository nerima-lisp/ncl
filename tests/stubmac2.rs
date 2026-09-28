#![allow(missing_docs)]

use std::process::Command;

fn eval(source: &str) -> (i32, String, String) {
    let Ok(output) = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
    else {
        return (
            -1,
            String::new(),
            "ncl process could not be started".to_owned(),
        );
    };
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn assert_returns_nil_for_a_true_test() {
    let (status, stdout, stderr) = eval("(assert t)");
    assert_eq!(status, 0, "{stderr}");
    assert_eq!(stdout.trim(), "NIL");
}

#[test]
fn multiple_value_setq_assigns_and_returns_primary_value() {
    let (status, stdout, stderr) =
        eval("(progn (setq a 0 b 0) (multiple-value-setq (a b) (values 7 9)) (list a b))");
    assert_eq!(status, 0, "{stderr}");
    assert_eq!(stdout.trim(), "(7 9)");
}

#[test]
fn remaining_macro_expanders_report_their_downstream_missing_runtime() {
    for (name, source, expected) in [
        ("CHECK-TYPE", "(check-type 1 string)", "Unsupported"),
        ("DECLAIM", "(declaim (special *x*))", "PROCLAIM"),
        (
            "RESTART-CASE",
            "(restart-case 1 (retry () 2))",
            "RESTART-BIND",
        ),
    ] {
        let (status, _stdout, stderr) = eval(source);
        assert_ne!(status, 0, "{name} unexpectedly succeeded");
        assert!(stderr.contains(expected), "{name}: {stderr}");
    }
}

#![allow(missing_docs)]
#![allow(clippy::expect_used)]

use std::process::{Command, Output};

fn eval(source: &str) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .expect("failed to run ncl");
    assert!(output.status.code().is_some(), "ncl terminated by signal");
    output
}

#[test]
fn undefined_function_paths_report_the_symbol_name() {
    for source in [
        "(no-such-function 1)",
        "(no-such-function 1 2 3 4 5)",
        "(funcall 'no-such-function)",
        "(apply 'no-such-function '(1))",
    ] {
        let result = eval(source);
        assert!(!result.status.success(), "{source}: {result:?}");
        assert_eq!(result.status.code(), Some(1), "{source}: {result:?}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("UndefinedFunction") || stderr.contains("NO-SUCH-FUNCTION"),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn fboundp_reports_an_undefined_function_as_unbound() {
    let result = eval("(fboundp 'no-such-function)");
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "NIL");
}

#[test]
fn fmakunbound_restores_the_undefined_function_cell() {
    let result = eval("(progn (defun g () 1) (fmakunbound 'g) (g))");
    assert!(result.status.code().is_some(), "{result:?}");
    assert!(!result.status.success(), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains('G'),
        "{result:?}"
    );
}

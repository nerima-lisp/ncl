#![allow(missing_docs)]

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
        "(funcall 'no-such-function)",
        "(apply 'no-such-function '(1))",
    ] {
        let result = eval(source);
        assert!(!result.status.success(), "{source}: {result:?}");
        assert_eq!(result.status.code(), Some(1), "{source}: {result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("NO-SUCH-FUNCTION"),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn undefined_function_is_caught_and_fboundp_is_nil() {
    let result = eval(
        "(handler-case (no-such-function) (undefined-function (condition) (cell-error-name condition)))",
    );
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "NO-SUCH-FUNCTION"
    );

    let result = eval("(fboundp 'no-such-function)");
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "NIL");
}

#[test]
fn fmakunbound_restores_the_undefined_function_cell() {
    let result = eval(
        "(progn (defun g () 1) (fmakunbound 'g) (handler-case (g) (undefined-function () :gone)))",
    );
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), ":GONE");
}

#[test]
fn five_argument_undefined_function_is_not_a_native_crash() {
    let result = eval("(no-such-function 1 2 3 4 5)");
    assert!(result.status.code().is_some(), "{result:?}");
    assert!(!result.status.success(), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("NO-SUCH-FUNCTION"),
        "{result:?}"
    );
}

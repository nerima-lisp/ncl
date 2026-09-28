#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

struct Case {
    name: &'static str,
    source: &'static str,
    stdout: &'static str,
}

struct XFail {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
}

fn run_ncl(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{source}: failed to run ncl: {error}"));
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .unwrap_or_else(|error| panic!("{source}: failed to collect output: {error}"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child
                    .kill()
                    .unwrap_or_else(|error| panic!("{source}: failed to stop timeout: {error}"));
                let _ = child.wait();
                panic!("{source}: ncl did not exit within {CHILD_TIMEOUT:?}");
            }
            Err(error) => panic!("{source}: failed to poll ncl: {error}"),
        }
    }
}

#[test]
fn compiled_clos_baseline_asserts_ansi_output() {
    let cases = [
        Case {
            name: "class-of-integer",
            source: "(class-name (class-of 3))",
            stdout: "\"INTEGER\"",
        },
        Case {
            name: "class-of-nil",
            source: "(class-name (class-of nil))",
            stdout: "\"NULL\"",
        },
    ];
    for case in cases {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(0), "{}", case.name);
        assert!(
            output.stderr.is_empty(),
            "{}: {:?}",
            case.name,
            output.stderr
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            case.stdout,
            "{}",
            case.name
        );
    }
}

#[test]
fn unsupported_compiled_clos_cases_remain_explicit_xfails() {
    let cases = [
        XFail {
            name: "call-next-method-outside-method",
            source: "(call-next-method)",
            stderr: "UNDEFINED-FUNCTION",
        },
        XFail {
            name: "unknown-generic",
            source: "(clos-unknown-generic 1)",
            stderr: "UNDEFINED-FUNCTION",
        },
    ];
    for case in cases {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(1), "{}", case.name);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: stderr={:?}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn compiled_clos_call_next_method_combines_primary_methods() {
    let output = run_ncl(
        "(progn (defclass point () ()) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) 2) (defmethod area ((s colored)) (+ 3 (call-next-method))) (area (make-instance 'colored)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "5");
}

#[test]
fn compiled_clos_method_combination_and_next_method_p_are_observable() {
    let output = run_ncl(
        "(progn (defgeneric combine (x)) (defmethod combine :around ((x integer)) (+ 100 (call-next-method))) (defmethod combine :before ((x integer)) (set 'clos-before 4)) (defmethod combine :after ((x integer)) (set 'clos-after 8)) (defmethod combine ((x integer)) (if (next-method-p) 0 3)) (list (combine 1) clos-before clos-after))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "(103 4 8)"
    );
}

#[test]
fn compiled_clos_class_and_instance_paths_are_available() {
    let output = run_ncl(
        "(progn (defclass point () ((x :initarg :x :accessor px) (y :initarg :y :initform 0 :accessor py))) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) (* (px s) (py s))) (area (make-instance 'colored :x 3)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "0");
}

#[test]
fn compiled_clos_quote_keeps_call_next_method_as_data() {
    let output = run_ncl(
        "(progn (defgeneric quoted-probe (x)) (defmethod quoted-probe ((x integer)) (car (list 'call-next-method))) (quoted-probe 1))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "COMMON-LISP-USER:CALL-NEXT-METHOD"
    );
}

#[test]
fn compiled_clos_throw_passes_through_call_next_method() {
    let output = run_ncl(
        "(catch 'k (progn (defgeneric throw-probe (s)) (defmethod throw-probe ((s integer)) (throw 'k 42)) (defmethod throw-probe :around ((s integer)) (+ 1000 (call-next-method))) (throw-probe 3)))",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "42");
}

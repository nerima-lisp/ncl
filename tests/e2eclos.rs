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
    let cases = [XFail {
        name: "unknown-generic",
        source: "(clos-unknown-generic 1)",
        stderr: "UNDEFINED-FUNCTION",
    }];
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
fn compiled_clos_class_and_instance_paths_are_available() {
    let cases = [
        (
            "(class-name (find-class 'standard-object))",
            "\"STANDARD-OBJECT\"",
        ),
        (
            "(class-name (class-of (make-instance 'standard-object)))",
            "\"STANDARD-OBJECT\"",
        ),
        (
            "(progn (defclass point () ((x :initarg :x) (y :initarg :y :initform 0))) (slot-value (make-instance 'point :x 3) 'y))",
            "0",
        ),
        (
            "(progn (defclass point () ((x :initarg :x :accessor px) (y :initarg :y :initform 0 :accessor py))) (px (make-instance 'point :x 3)))",
            "3",
        ),
        (
            "(progn (defclass point () ((x :initarg :x :accessor px) (y :initarg :y :initform 0 :accessor py))) (defclass colored (point) ()) (typep (make-instance 'colored :x 3) 'point))",
            "T",
        ),
        (
            "(progn (set 'area 77) (defgeneric area (s)) (symbol-value 'area))",
            "77",
        ),
        (
            "(progn (defclass point () ()) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) 2) (defmethod area ((s colored)) 3) (area (make-instance 'colored)))",
            "3",
        ),
        (
            "(progn (defgeneric classify (x y)) (defmethod classify ((x integer) (y integer)) 1) (classify 1 2))",
            "1",
        ),
        (
            "(progn (defgeneric exact (x)) (defmethod exact ((x integer)) 1) (defmethod exact ((x (eql 3))) 9) (exact 3))",
            "9",
        ),
        (
            "(progn (defgeneric redefine (x)) (defmethod redefine ((x integer)) 1) (defmethod redefine ((x integer)) 2) (redefine 3))",
            "2",
        ),
        (
            "(progn (defclass point () ((x :initarg :x :accessor px) (y :initarg :y :initform 0 :accessor py))) (defclass colored (point) ()) (defgeneric area (s)) (defmethod area ((s point)) (* (px s) (py s))) (area (make-instance 'colored :x 3)))",
            "0",
        ),
    ];
    for (source, expected) in cases {
        let output = run_ncl(source);
        assert_eq!(output.status.code(), Some(0), "{source}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            expected,
            "{source}"
        );
    }
}

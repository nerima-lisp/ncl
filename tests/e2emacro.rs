#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Probe {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

struct XFail {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
    exit_code: i32,
}

const PROBES: &[Probe] = &[
    Probe {
        name: "when",
        source: "(when t 7)",
        expected: "7",
    },
    Probe {
        name: "unless",
        source: "(unless nil 8)",
        expected: "8",
    },
    Probe {
        name: "cond",
        source: "(cond (nil 1) (t 2))",
        expected: "2",
    },
    Probe {
        name: "and",
        source: "(and t 3)",
        expected: "3",
    },
    Probe {
        name: "or",
        source: "(or nil 4)",
        expected: "4",
    },
    Probe {
        name: "case",
        source: "(case 2 (1 10) (2 20))",
        expected: "20",
    },
    Probe {
        name: "case-default",
        source: "(case 3 (1 10) (otherwise 30))",
        expected: "30",
    },
    Probe {
        name: "prog1",
        source: "(prog1 7 8)",
        expected: "7",
    },
    Probe {
        name: "prog2",
        source: "(prog2 7 8 9)",
        expected: "8",
    },
    Probe {
        name: "loop-return",
        source: "(loop repeat 3 return 9)",
        expected: "9",
    },
    Probe {
        name: "multiple-value-bind",
        source: "(multiple-value-bind (a b) (values 1 2) (+ a b))",
        expected: "3",
    },
    Probe {
        name: "multiple-value-setq",
        source: "(progn (setq a 0 b 0) (multiple-value-setq (a b) (values 7 9)))",
        expected: "7",
    },
    Probe {
        name: "with-input-from-string",
        source: "(with-input-from-string (s \"abc\") (read-char s))",
        expected: "#\\a",
    },
    Probe {
        name: "with-output-to-string",
        source: "(with-output-to-string (s) (write-char #\\a s))",
        expected: "\"a\"",
    },
    Probe {
        name: "with-output-to-string-optional-string",
        source: "(with-output-to-string (s \"a\" :element-type 'character) (write-char #\\b s))",
        expected: "\"ab\"",
    },
    Probe {
        name: "with-output-to-string-keyword-only",
        source: "(with-output-to-string (s :element-type 'character) (write-char #\\c s))",
        expected: "\"c\"",
    },
    Probe {
        name: "typecase-type-test",
        source: "(typecase nil (integer 11) (otherwise 12))",
        expected: "12",
    },
];

const XFAILS: &[XFail] = &[
    XFail {
        name: "loop-in",
        source: "(loop for x in (quote (1 2 3)) collect x)",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-on",
        source: "(loop for x on (quote (1 2 3)) collect (car x))",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-from-below-by",
        source: "(loop for x from 1 below 4 by 2 sum x)",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-count",
        source: "(loop for x from 1 below 4 count (oddp x))",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-maximize",
        source: "(loop for x from 1 below 4 maximize x)",
        stderr: "UnsupportedLiteral",
        exit_code: 1,
    },
    XFail {
        name: "loop-while",
        source: "(loop for x from 1 below 4 while (< x 3) collect x)",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-until",
        source: "(loop for x from 1 below 4 until (= x 3) collect x)",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "loop-with-finally",
        source: "(loop with x = 2 finally (return x))",
        stderr: "SafepointWarning",
        exit_code: 1,
    },
    XFail {
        name: "loop-nested",
        source: "(loop for x from 1 below 3 collect (loop for y from 1 below 3 sum (+ x y)))",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "destructuring-bind",
        source: "(destructuring-bind (a (b &optional c) &rest d) (list 1 (list 2 3) 4 5) (list a b c d))",
        stderr: "UnknownLambdaListKeyword",
        exit_code: 1,
    },
    XFail {
        name: "do",
        source: "(do ((x 0 (1+ x))) ((= x 3) x))",
        stderr: "PSETQ",
        exit_code: 1,
    },
    XFail {
        name: "do-star",
        source: "(do* ((x 0 (1+ x))) ((= x 3) x))",
        stderr: "UndefinedValue",
        exit_code: 1,
    },
    XFail {
        name: "dolist",
        source: "(dolist (x (list 1 2 3) 9) x)",
        stderr: "SafepointWarning",
        exit_code: 1,
    },
    XFail {
        name: "dotimes",
        source: "(dotimes (x 3 9) x)",
        stderr: "SafepointWarning",
        exit_code: 1,
    },
    XFail {
        name: "defstruct",
        source: "(progn (defstruct point x y) (point-x (make-point :x 3 :y 4)))",
        stderr: "DEFSTRUCT",
        exit_code: 1,
    },
    XFail {
        name: "psetq",
        source: "(let ((a 1) (b 2)) (psetq a b b a) (list a b))",
        stderr: "PSETQ",
        exit_code: 1,
    },
    XFail {
        name: "rotatef",
        source: "(let ((a 1) (b 2)) (rotatef a b) (list a b))",
        stderr: "ROTATEF",
        exit_code: 1,
    },
    XFail {
        name: "shiftf",
        source: "(let ((a 1) (b 2)) (shiftf a b 3) (list a b))",
        stderr: "SHIFTF",
        exit_code: 1,
    },
];

fn run_ncl(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .unwrap_or_else(|error| panic!("{source}: {error}"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child
                    .kill()
                    .unwrap_or_else(|error| panic!("{source}: timeout kill failed: {error}"));
                let _ = child.wait();
                panic!("{source}: ncl did not exit within 30 seconds");
            }
            Err(error) => panic!("{source}: poll failed: {error}"),
        }
    }
}

#[test]
fn standard_macro_probes_assert_compiled_results() {
    for probe in PROBES {
        let output = run_ncl(probe.source);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {:?}",
            probe.name,
            output
        );
        assert!(output.stderr.is_empty(), "{}: {:?}", probe.name, output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            probe.expected,
            "{}",
            probe.name
        );
    }
}

#[test]
fn unsupported_standard_macro_paths_remain_explicit_xfails() {
    for case in XFAILS {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(case.exit_code), "{}", case.name);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: {:?}",
            case.name,
            output
        );
    }
}

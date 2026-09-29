#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

struct Case {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "mapcar throw",
        source: "(catch 'x (mapcar (lambda (v) (throw 'x v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "mapcar return-from",
        source: "(block b (mapcar (lambda (v) (return-from b v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "mapcar go",
        source: "(progn (setq *nlx-go* 0) (tagbody (mapcar (lambda (v) (go out)) '(1)) (setq *nlx-go* 99) out) *nlx-go*)",
        expected: "0",
    },
    Case {
        name: "mapc throw",
        source: "(catch 'x (mapc (lambda (v) (throw 'x v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "maplist throw",
        source: "(catch 'x (maplist (lambda (v) (throw 'x v)) '(1 2)))",
        expected: "(1 2)",
    },
    Case {
        name: "mapcan throw",
        source: "(catch 'x (mapcan (lambda (v) (throw 'x v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "reduce throw",
        source: "(catch 'x (reduce (lambda (a b) (throw 'x (+ a b))) '(1 2 3)))",
        expected: "3",
    },
    Case {
        name: "sort predicate throw",
        source: "(catch 'x (sort (list 3 1 2) (lambda (a b) (throw 'x a))))",
        expected: "1",
    },
    Case {
        name: "sort key throw",
        source: "(catch 'x (sort (list 3 1 2) #'< :key (lambda (v) (throw 'x v))))",
        expected: "3",
    },
    Case {
        name: "maphash throw",
        source: "(catch 'x (let ((h (make-hash-table))) (setf (gethash 1 h) 2) (maphash (lambda (k v) (throw 'x v)) h)))",
        expected: "2",
    },
    Case {
        name: "apply throw",
        source: "(catch 'x (apply (lambda (v) (throw 'x v)) '(4)))",
        expected: "4",
    },
    Case {
        name: "builtin funcall throw",
        source: "(catch 'x (funcall #'funcall (lambda () (throw 'x 3))))",
        expected: "3",
    },
    Case {
        name: "some throw",
        source: "(catch 'x (some (lambda (v) (throw 'x v)) '(1 2)))",
        expected: "1",
    },
    Case {
        name: "find test throw",
        source: "(catch 'x (find 1 '(1 2) :test (lambda (a b) (throw 'x b))))",
        expected: "1",
    },
    Case {
        name: "member test throw",
        source: "(catch 'x (member 1 '(1 2) :test (lambda (a b) (throw 'x b))))",
        expected: "1",
    },
    Case {
        name: "assoc key throw",
        source: "(catch 'x (assoc 1 '((1 . 2)) :key (lambda (a) (throw 'x a))))",
        expected: "1",
    },
    Case {
        name: "union test throw",
        source: "(catch 'x (union '(1) '(2) :test (lambda (a b) (throw 'x a))))",
        expected: "1",
    },
    Case {
        name: "unwind-protect around callback",
        source: "(progn (setq *nlx-c* 0) (setq *nlx-r* (catch 'x (unwind-protect (mapcar (lambda (v) (throw 'x v)) '(5 6)) (setq *nlx-c* 1)))) (list *nlx-r* *nlx-c*))",
        expected: "(5 1)",
    },
    Case {
        name: "unwind-protect inside callback",
        source: "(progn (setq *nlx-c* 0) (setq *nlx-r* (catch 'x (mapcar (lambda (v) (unwind-protect (throw 'x v) (setq *nlx-c* (+ *nlx-c* 10)))) '(1 2)))) (list *nlx-r* *nlx-c*))",
        expected: "(1 10)",
    },
    Case {
        name: "mismatched inner catch",
        source: "(catch 'x (catch 'y (mapcar (lambda (v) (throw 'x v)) '(3))))",
        expected: "3",
    },
    Case {
        name: "stale marker regression",
        source: "(catch 'outer (list (catch 'y (mapcar (lambda (v) (throw 'y v)) '(1))) (car '(2))))",
        expected: "(1 2)",
    },
    Case {
        name: "gc stress fresh throw value",
        source: "(catch 'x (mapcar (lambda (v) (unwind-protect (throw 'x (list v v)) (list v v))) '(1 2)))",
        expected: "(1 1)",
    },
];

fn run(source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl: {error}"))
}

#[test]
fn native_non_local_exit_cli_cases() {
    assert!(!CASES.is_empty(), "native NLX case table must not be empty");
    for case in CASES {
        let output = run(case.source);
        assert_eq!(output.status.code(), Some(0), "{}: {output:?}", case.name);
        assert!(output.stderr.is_empty(), "{}: {:?}", case.name, output.stderr);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            case.expected,
            "{}",
            case.name
        );
    }
}

#[test]
fn uncaught_callback_exit_remains_a_control_error() {
    let output = run("(mapcar (lambda (v) (throw 'nope v)) '(1))");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ControlError"), "{stderr}");
    assert!(!stderr.contains("NonLocalExit"), "{stderr}");
}

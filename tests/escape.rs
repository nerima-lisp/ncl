#![allow(missing_docs)]

use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

struct Case {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

fn run_ncl(source: &str) -> Output {
    let child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{source}: failed to run ncl: {error}"));
    wait_with_timeout(child, source)
}

fn wait_with_timeout(mut child: Child, source: &str) -> Output {
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
#[allow(
    clippy::too_many_lines,
    reason = "table keeps the escape matrix readable"
)]
fn non_local_escape_values_are_observable_through_the_cli() {
    let cases = [
        Case {
            name: "flet-return-from",
            source: "(block done (flet ((escape () (return-from done 11))) (escape) 99))",
            expected: "11",
        },
        Case {
            name: "labels-return-from",
            source: "(block done (labels ((escape () (return-from done 12))) (escape) 99))",
            expected: "12",
        },
        Case {
            name: "labels-self-recursion-return-from",
            source: "(block done (labels ((walk (n) (if (= n 0) (return-from done 21) (walk (- n 1))))) (walk 3) 99))",
            expected: "21",
        },
        Case {
            name: "mapcar-go",
            source: "(progn (setq *nlx-go* 0) (tagbody (mapcar (lambda (v) (go out)) '(1)) (setq *nlx-go* 99) out) *nlx-go*)",
            expected: "0",
        },
        Case {
            name: "mapcar-return-from",
            source: "(block b (mapcar (lambda (v) (return-from b v)) '(1 2)))",
            expected: "1",
        },
        Case {
            name: "lambda-return-from",
            source: "(block done (funcall (lambda () (return-from done 18))) 99)",
            expected: "18",
        },
        Case {
            name: "nested-handler-flet-return-from",
            source: "(block aborted (let ((result nil)) (handler-bind nil (flet ((run () (handler-bind ((error (lambda (condition) (setf result (list condition)) (return-from aborted nil)))) (error \"stop\")))) (run))) result))",
            expected: "NIL",
        },
        Case {
            name: "lambda-go",
            source: "(let ((result 0)) (tagbody (funcall (lambda () (go done))) done (setq result 13)) result)",
            expected: "13",
        },
        Case {
            name: "flet-go",
            source: "(let ((result 0)) (tagbody (flet ((jump () (go done))) (jump)) done (setq result 19)) result)",
            expected: "19",
        },
        Case {
            name: "labels-go",
            source: "(let ((result 0)) (tagbody (labels ((jump () (go done))) (jump)) done (setq result 20)) result)",
            expected: "20",
        },
        Case {
            name: "return-from-multiple-values",
            source: "(multiple-value-list (block done (return-from done (values 1 2))))",
            expected: "(1 2)",
        },
        Case {
            name: "unwind-protect-cleanup",
            source: "(progn (setq *escape-cleanup* 0) (block done (unwind-protect (return-from done 14) (setq *escape-cleanup* 15))) *escape-cleanup*)",
            expected: "15",
        },
        Case {
            name: "outer-frame-closure",
            source: "(block done (funcall (lambda (n) (return-from done 16)) 3) 99)",
            expected: "16",
        },
        Case {
            name: "gc-stress-allocation-pressure",
            source: "(block done (funcall (lambda () (return-from done (length (list (list 1 2 3) (list 4 5 6) (list 7 8 9)))))))",
            expected: "3",
        },
        Case {
            name: "block-assignment-survives-normal-exit",
            source: "(let ((x 0)) (block b (setq x 5) (return-from b 9)) x)",
            expected: "5",
        },
        Case {
            name: "block-assignment-preserves-unassigned-value",
            source: "(let ((x 0) (y 0)) (block b (setq x 1) (when t (return-from b nil)) (setq y 2)) (list x y))",
            expected: "(1 0)",
        },
        Case {
            name: "flet-assignment-through-closure",
            source: "(let ((x 0)) (flet ((bump () (setq x (1+ x)))) (bump) (bump)) x)",
            expected: "2",
        },
        Case {
            name: "block-lambda-return-preserves-assignment",
            source: "(let ((x 0)) (block b (setq x 1) (funcall (lambda () (return-from b nil))) (setq x 2)) x)",
            expected: "1",
        },
        Case {
            name: "block-mapcar-return-preserves-unassigned-value",
            source: "(let ((x 0) (y 0)) (block b (setq x 1) (mapcar (lambda (v) (return-from b v)) '(7)) (setq y 2)) (list x y))",
            expected: "(1 0)",
        },
        Case {
            name: "tagbody-assignment-through-lambda-go",
            source: "(let ((x 0)) (tagbody (setq x 5) (funcall (lambda () (go done))) done) x)",
            expected: "5",
        },
        Case {
            name: "tagbody-mapcar-go-preserves-unassigned-value",
            source: "(let ((x 0) (y 0)) (tagbody (setq x 1) (mapcar (lambda (v) (go done)) '(7)) (setq y 2) done) (list x y))",
            expected: "(1 0)",
        },
        Case {
            name: "dotimes-value-loop",
            source: "(dotimes (i 5) i)",
            expected: "NIL",
        },
        Case {
            name: "dotimes-setq-loop",
            source: "(let ((s 0)) (dotimes (i 10) (setq s (+ s i))) s)",
            expected: "45",
        },
        Case {
            name: "tagbody-backedge-loop",
            source: "(let ((x 0)) (tagbody top (setq x (1+ x)) (when (< x 5) (go top))) x)",
            expected: "5",
        },
        Case {
            name: "tagbody-go-preserves-unassigned-value",
            source: "(let ((x 0) (y 0)) (tagbody (setq x 1) (when t (go end)) (setq y 2) end) (list x y))",
            expected: "(1 0)",
        },
        Case {
            name: "do-parallel-updates",
            source: "(do ((i 0 (1+ i)) (acc nil (cons i acc))) ((= i 3) acc))",
            expected: "(2 1 0)",
        },
        Case {
            name: "dotimes-closures-do-not-crash",
            source: "(let ((fs nil)) (dotimes (i 3) (push (lambda () i) fs)) (mapcar #'funcall fs))",
            expected: "(3 3 3)",
        },
    ];

    for case in cases {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(0), "{}: {:?}", case.name, output);
        assert!(
            output.stderr.is_empty(),
            "{}: {:?}",
            case.name,
            output.stderr
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            case.expected,
            "{}",
            case.name
        );
    }
}

#[test]
fn loops_with_tagbody_and_closures_survive_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    for (source, expected) in [
        ("(let ((s 0)) (dotimes (i 10) (setq s (+ s i))) s)", "45"),
        (
            "(let ((x 0)) (tagbody top (setq x (1+ x)) (when (< x 5) (go top))) x)",
            "5",
        ),
    ] {
        let value = runtime
            .compile(source)
            .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
        assert_eq!(runtime.format_result(value), expected, "{source}");
    }
}

#[test]
fn expired_closure_reports_control_error_through_the_cli() {
    let cases = [
        "(progn (setq *escape-closure* nil) (block done (setq *escape-closure* (lambda () (return-from done 17)))) (funcall *escape-closure*))",
        "(let ((f nil)) (block b (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil) (n 0)) (block b (setq n 3) (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil) (n 0) (m 0)) (block b (setq n 3) (setq m 4) (setq f (lambda () (return-from b 1)))) (funcall f))",
        "(let ((f nil)) (block b (flet ((make () (lambda () (return-from b 1)))) (setq f (make)))) (funcall f))",
        "(let ((f nil)) (block b (labels ((make () (lambda () (return-from b 1)))) (setq f (make)))) (funcall f))",
        "(let ((f nil) (n 0)) (tagbody (setq n 3) (setq f (lambda () (go done))) done) (funcall f))",
        "(let ((f nil)) (tagbody (setq f (flet ((jump () (go done))) (lambda () (jump)))) done) (funcall f))",
        "(let ((f nil)) (tagbody (setq f (labels ((jump () (go done))) (lambda () (jump)))) done) (funcall f))",
    ];
    for source in cases {
        let output = run_ncl(source);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("ControlError"),
            "{source}: stderr={:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

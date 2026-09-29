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
fn expired_closure_reports_control_error_through_the_cli() {
    let source = "(progn (setq *escape-closure* nil) (block done (setq *escape-closure* (lambda () (return-from done 17)))) (funcall *escape-closure*))";
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ControlError"),
        "stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

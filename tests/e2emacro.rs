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

/// A form that must fail: unlike [`XFail`], this is the *correct*, permanent
/// outcome (an unhandled condition), not a gap to eventually close.
struct ErrorCase {
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
        name: "loop-in-now-lowers",
        source: "(loop for x in (quote (1 2 3)) collect x)",
        expected: "(1 2 3)",
    },
    Probe {
        name: "multiple-value-bind",
        source: "(multiple-value-bind (a b) (values 1 2) (+ a b))",
        expected: "3",
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
    // B1: backquote (CLHS 2.4.6), formerly `InvalidOperator`.
    Probe {
        name: "backquote-unquote-splice",
        source: "`(1 ,(+ 1 1) ,@(list 3 4))",
        expected: "(1 2 3 4)",
    },
    Probe {
        name: "backquote-dotted-unquote",
        source: "`(a . ,(+ 1 2))",
        expected: "(COMMON-LISP-USER:A . 3)",
    },
    Probe {
        name: "backquote-splice-middle",
        source: "`(1 ,@(list 2 3) 4)",
        expected: "(1 2 3 4)",
    },
    // Nested backquote (CLHS 2.4.6): the inner `,,x` fires at the outer
    // level, the outer backquote itself only reconstructs as data.
    Probe {
        name: "backquote-nested",
        source: "(let ((x 5)) ``(a ,,x))",
        expected: "(QUASIQUOTE (COMMON-LISP-USER:A (UNQUOTE 5)))",
    },
    Probe {
        name: "backquote-vector",
        source: "`#(1 ,(+ 1 1) ,@(list 3 4))",
        expected: "#(1 2 3 4)",
    },
    Probe {
        name: "array-literal-rank-two",
        source: "(aref #2A((1 2) (3 4)) 1 0)",
        expected: "3",
    },
    Probe {
        name: "backquote-in-defmacro-body",
        source: "(progn (defmacro my-add (a b) `(+ ,a ,b)) (my-add 1 2))",
        expected: "3",
    },
    // B2: destructuring-bind (CLHS 3.4.5), formerly silently wrong.
    Probe {
        name: "destructuring-bind-simple",
        source: "(destructuring-bind (a b) (list 1 2) (list a b))",
        expected: "(1 2)",
    },
    Probe {
        name: "destructuring-bind-single",
        source: "(destructuring-bind (a) (list 1) a)",
        expected: "1",
    },
    // Formerly XFAIL `UnknownLambdaListKeyword`; nested pattern + &optional + &rest.
    Probe {
        name: "destructuring-bind-nested-optional-rest",
        source: "(destructuring-bind (a (b &optional c) &rest d) (list 1 (list 2 3) 4 5) (list a b c d))",
        expected: "(1 2 3 (4 5))",
    },
    Probe {
        name: "destructuring-bind-nested-key",
        source: "(destructuring-bind (a (b c) &key d) '(1 (2 3) :d 4) (list a b c d))",
        expected: "(1 2 3 4)",
    },
    Probe {
        name: "destructuring-bind-dotted",
        source: "(destructuring-bind (a . b) '(1 2 3) (list a b))",
        expected: "(1 (2 3))",
    },
    Probe {
        name: "destructuring-bind-key-default-supplied-p",
        source: "(destructuring-bind (&key (x 10 x-p)) '() (list x x-p))",
        expected: "(10 NIL)",
    },
    // B3: defmacro lambda lists (CLHS 3.4.4): &body, &whole, &environment, nesting.
    Probe {
        name: "defmacro-body-keyword",
        source: "(progn (defmacro my-progn (&body forms) `(progn ,@forms)) (my-progn 1 2 3))",
        expected: "3",
    },
    Probe {
        name: "defmacro-nested-pattern",
        source: "(progn (defmacro my-swap ((a b)) `(list ,b ,a)) (my-swap (1 2)))",
        expected: "(2 1)",
    },
    Probe {
        name: "defmacro-whole",
        source: "(progn (defmacro show-whole (&whole form a) `(list ',form ,a)) (show-whole 9))",
        expected: "((COMMON-LISP-USER:SHOW-WHOLE 9) 9)",
    },
    Probe {
        name: "defmacro-optional-key-defaults",
        source: "(progn (defmacro my-opt (a &optional (b 10) &key (c 20)) `(list ,a ,b ,c)) (my-opt 1))",
        expected: "(1 10 20)",
    },
    // B4: macrolet local macros (CLHS special operator MACROLET).
    Probe {
        name: "macrolet-basic",
        source: "(macrolet ((double (x) `(* 2 ,x))) (double 5))",
        expected: "10",
    },
    Probe {
        name: "macrolet-shadows-global",
        source: "(progn (defmacro shadowed () 1) (macrolet ((shadowed () 2)) (shadowed)))",
        expected: "2",
    },
    Probe {
        name: "symbol-macrolet-respects-lexical-shadowing",
        source: "(symbol-macrolet ((x 7)) (list x (let ((x 2)) x)))",
        expected: "(7 2)",
    },
    // B7: ECASE/CCASE signal a type-error on no match (was: silently NIL).
    Probe {
        name: "ecase-match",
        source: "(ecase 2 (1 10) (2 20))",
        expected: "20",
    },
    // Regression: ECASE's no-match branch used to build its condition's
    // :DATUM/:EXPECTED-TYPE keyword arguments from malformed pseudo-keyword
    // symbols (interned as literal ":DATUM" text in COMMON-LISP instead of
    // KEYWORD::DATUM), so the signalled error bypassed the condition system
    // and HANDLER-CASE could not catch it as a TYPE-ERROR.
    Probe {
        name: "ecase-no-match-is-a-catchable-type-error",
        source: "(handler-case (ecase 5 (1 'a) (2 'b)) (type-error (c) (type-error-datum c)))",
        expected: "5",
    },
    Probe {
        name: "ccase-no-match-is-a-catchable-type-error",
        source: "(handler-case (ccase 5 (1 'a) (2 'b)) (type-error (c) (type-error-datum c)))",
        expected: "5",
    },
    Probe {
        name: "loop-count",
        source: "(loop for x from 1 below 4 count (oddp x))",
        expected: "2",
    },
    Probe {
        name: "loop-below-defaults-from-zero",
        source: "(loop for i below 3 collect i)",
        expected: "(0 1 2)",
    },
    Probe {
        name: "loop-to-defaults-from-zero",
        source: "(loop for i to 3 collect i)",
        expected: "(0 1 2 3)",
    },
    Probe {
        name: "loop-destructuring-for",
        source: "(loop for (a b) in '((1 2) (3 4)) collect (+ a b))",
        expected: "(3 7)",
    },
    Probe {
        name: "loop-when-collect",
        source: "(loop for i from 1 to 10 when (evenp i) collect i)",
        expected: "(2 4 6 8 10)",
    },
    Probe {
        name: "loop-unless-collect",
        source: "(loop for i from 1 to 5 unless (evenp i) collect i)",
        expected: "(1 3 5)",
    },
    Probe {
        name: "loop-when-it",
        source: "(loop for x in '(1 nil 2 nil 3) when x collect it)",
        expected: "(1 2 3)",
    },
];

const XFAILS: &[XFail] = &[
    XFail {
        name: "loop-maximize",
        source: "(loop for x from 1 below 4 maximize x)",
        stderr: "UnsupportedLiteral",
        exit_code: 1,
    },
    XFail {
        name: "loop-with-finally",
        source: "(loop with x = 2 finally (return x))",
        stderr: "UndefinedValue",
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
    XFail {
        name: "multiple-value-setq",
        source: "(multiple-value-setq (a b) (values 1 2))",
        stderr: "MULTIPLE-VALUE-SETQ",
        exit_code: 1,
    },
];

const ERROR_CASES: &[ErrorCase] = &[
    // B7: ECASE, unlike CASE, signals when nothing matches.
    ErrorCase {
        name: "ecase-no-match-signals",
        source: "(ecase 5 (1 'a) (2 'b))",
        stderr: "Unsupported",
        exit_code: 1,
    },
    // B7: CCASE gets the same treatment as ECASE.
    ErrorCase {
        name: "ccase-no-match-signals",
        source: "(ccase 5 (1 'a) (2 'b))",
        stderr: "Unsupported",
        exit_code: 1,
    },
    // B2: a shape mismatch signals instead of binding garbage (was
    // `((1 2) 48)` for the two-argument case; see `destructuring-bind-*`
    // probes above for the value-correct cases).
    ErrorCase {
        name: "destructuring-bind-too-few-signals",
        source: "(destructuring-bind (a b) (list 1) (list a b))",
        stderr: "too few elements",
        exit_code: 1,
    },
    ErrorCase {
        name: "destructuring-bind-too-many-signals",
        source: "(destructuring-bind (a) (list 1 2) a)",
        stderr: "too many elements",
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
fn macro_forms_that_must_error_do_error() {
    for case in ERROR_CASES {
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

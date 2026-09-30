#![allow(missing_docs)]
//! Regression tests for P1-ctl: `IF`/`WHEN`/`UNLESS`/`COND` branch-merge
//! (A1, `lower_if`), non-tail non-local-exit call arguments (A4, `lower_call`),
//! and the `LOOP` fixes (B6: default arithmetic-`FOR` init, `WHEN`/`UNLESS`/`IF`
//! clauses, destructuring `FOR` variables).

mod common;

use common::{assert_eval_with_stress, run_ncl};

struct Case {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    // A1: `if`/`when`/`unless`/`cond` must merge a variable assigned in
    // either branch back into a value later reads observe.
    Case {
        name: "if-both-branches-assign-same-variable",
        source: "(let ((x 0)) (if t (setq x 1) (setq x 2)) (1+ x))",
        expected: "2",
    },
    Case {
        name: "when-assigns-outer-variable",
        source: "(let ((x 0)) (when (> 1 0) (setq x 5)) x)",
        expected: "5",
    },
    Case {
        name: "if-untaken-branch-variable-keeps-pre-branch-value",
        source: "(let ((x 0) (y 0)) (if nil (setq x 1) (setq y 2)) (list x y))",
        expected: "(0 2)",
    },
    Case {
        name: "cond-three-branches-assign-different-variables",
        source: "(let ((x 0) (y 0) (z 0)) (cond ((= 1 2) (setq x 1)) ((= 1 1) (setq y 2)) (t (setq z 3))) (list x y z))",
        expected: "(0 2 0)",
    },
    Case {
        name: "unless-assigns-outer-variable",
        source: "(let ((x 0)) (unless nil (setq x 9)) x)",
        expected: "9",
    },
    Case {
        name: "nested-if-assigns-through-both-levels",
        source: "(let ((x 0)) (if t (if nil (setq x 1) (setq x 2)) (setq x 3)) x)",
        expected: "2",
    },
    Case {
        name: "dolist-if-push-preserves-order",
        source: "(let ((acc nil)) (dolist (e '(1 2 3)) (if (oddp e) (push e acc))) acc)",
        expected: "(3 1)",
    },
    Case {
        name: "while-loop-if-setq-accumulates",
        source: "(let ((i 0) (sum 0)) (loop while (< i 5) do (if (evenp i) (setq sum (+ sum i))) (setq i (+ i 1))) sum)",
        expected: "6",
    },
    Case {
        name: "tagbody-go-if-setq-merges-across-backedge",
        source: "(let ((i 0) (s 0)) (tagbody top (when (< i 5) (incf s i) (incf i) (go top))) s)",
        expected: "10",
    },
    // A4: a non-tail throw/go/return-from used as a call argument must stop
    // lowering later arguments instead of corrupting the terminated block.
    Case {
        name: "return-from-as-non-tail-call-argument",
        source: "(block b (list 1 (return-from b 7) 3))",
        expected: "7",
    },
    Case {
        name: "throw-as-non-tail-call-argument",
        source: "(catch 'k (+ 1 (throw 'k 5)))",
        expected: "5",
    },
    Case {
        name: "throw-as-non-tail-builtin-argument",
        source: "(catch 'x (list 1 (throw 'x 10)))",
        expected: "10",
    },
    // B6: LOOP default-from-zero, WHEN/UNLESS/IF, and destructuring FOR.
    Case {
        name: "loop-for-below-defaults-from-zero",
        source: "(loop for i below 3 collect i)",
        expected: "(0 1 2)",
    },
    Case {
        name: "loop-for-to-defaults-from-zero",
        source: "(loop for i to 3 collect i)",
        expected: "(0 1 2 3)",
    },
    Case {
        name: "loop-destructuring-for-variables",
        source: "(loop for (a b) in '((1 2) (3 4)) collect (+ a b))",
        expected: "(3 7)",
    },
    Case {
        name: "loop-when-collect",
        source: "(loop for i from 1 to 10 when (oddp i) collect i)",
        expected: "(1 3 5 7 9)",
    },
    Case {
        name: "loop-unless-collect",
        source: "(loop for i from 1 to 5 unless (oddp i) collect i)",
        expected: "(2 4)",
    },
    Case {
        name: "loop-if-else-into-finally",
        source: "(loop for i from 1 to 5 if (oddp i) collect i into odds else collect i into evens end finally (return (list (length odds) (length evens))))",
        expected: "(3 2)",
    },
    Case {
        name: "loop-when-it-binds-test-value",
        source: "(loop for x in '(1 nil 2 nil 3) when x collect it)",
        expected: "(1 2 3)",
    },
];

#[test]
fn control_flow_and_loop_regressions_hold_through_the_cli() {
    for case in CASES {
        let output = run_ncl(case.source);
        assert_eq!(output.status.code(), Some(0), "{}: {:?}", case.name, output);
        assert!(
            output.stderr.is_empty(),
            "{}: stderr={:?}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            case.expected,
            "{}",
            case.name
        );
    }
}

/// A subset of the battery, compiled in-process under `gc_stress` +
/// `strict_forwarding`: `lower_if`'s branch-merge threads GC-managed `Word`
/// values through SSA block parameters, so a moving collector must not
/// corrupt them.
///
/// Only plain `if`/`when`/`unless` forms are exercised here. `cond`, `dolist`,
/// `tagbody`+`go`, `loop`, and `catch`/`throw` are deliberately left out: a
/// full `ncl_runtime::Runtime::compile` of *any* of them under `gc_stress`
/// (including forms unrelated to this lane's changes, e.g. the pre-existing
/// `(loop for x in '(1 2 3) collect x)`) fails intermittently with
/// `Storage(ThreadNotRegistered)`/`TypeError`, while the identical logic
/// written as nested `if`s passes reliably. This is a pre-existing GC-root
/// gap in the macro-dispatch/expansion caller (suspected in the
/// `COMMON-LISP-USER`-inherited-macro branch of `expand_cons`,
/// `packages/compiler/front/src/expand/call.rs:122-141`, which reads `head`
/// again after an allocating `intern` call with no `with_root` protection),
/// not in `lower_if`/`lower_call`/`loop`; see the final report for detail.
/// It is out of this lane's scope and is not fixed here; `expand_loop_ast`
/// itself (this lane's LOOP changes) is separately proven GC-safe by
/// `packages/lib/macros/src/gc_stress_tests.rs`'s
/// `loop_conditional_clause_expansion_survives_gc_stress_and_strict_forwarding`.
#[test]
fn control_flow_and_loop_regressions_survive_gc_stress() {
    let gc_stress_safe = [
        "if-both-branches-assign-same-variable",
        "when-assigns-outer-variable",
        "if-untaken-branch-variable-keeps-pre-branch-value",
        "unless-assigns-outer-variable",
        "nested-if-assigns-through-both-levels",
        "return-from-as-non-tail-call-argument",
    ];
    for case in CASES {
        if gc_stress_safe.contains(&case.name) {
            assert_eval_with_stress(case.source, case.expected);
        }
    }
}

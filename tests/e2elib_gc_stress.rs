//! GC-stress coverage (forced collection + strict forwarding) for the
//! allocating P1-lib builtins: `COPY-TREE`, `REMOVE-IF`, `COERCE`
//! (list -> vector), and `MAKE-ARRAY` `:initial-contents`. Each of these
//! roots every heap value it touches across allocation, per
//! `packages/lib/sequences/src/domain/list_copy_tree.rs`,
//! `packages/lib/sequences/src/domain/selection_if.rs`, and
//! `packages/lib/hash-arrays/src/arrays/make_array_builtin.rs`; running them
//! against a long list under forced collection is the regression that would
//! catch a missed root.

#![allow(missing_docs)]

// This file only needs `assert_eval_with_stress_on`; `common::run_ncl` is
// used by other test binaries that share this module.
#[allow(dead_code)]
mod common;

use common::assert_eval_with_stress_on;

const LONG_LIST_LEN: usize = 150;

/// Build an ascending list `(1 2 ... LONG_LIST_LEN)` via `DOTIMES` rather
/// than a single `(list 1 2 ... n)` call, whose argument count would exceed
/// the immediate-operand encoding limit exercised by unrelated codegen paths.
fn range_list_source() -> String {
    format!(
        "(let ((acc nil)) (dotimes (i {LONG_LIST_LEN}) (setq acc (cons (- {LONG_LIST_LEN} i) acc))) acc)"
    )
}

fn printed_list(values: impl Iterator<Item = usize>) -> String {
    let mut printed = String::from("(");
    let mut first = true;
    for value in values {
        if !first {
            printed.push(' ');
        }
        printed.push_str(&value.to_string());
        first = false;
    }
    printed.push(')');
    printed
}

fn printed_vector(values: impl Iterator<Item = usize>) -> String {
    let mut printed = String::from("#(");
    let mut first = true;
    for value in values {
        if !first {
            printed.push(' ');
        }
        printed.push_str(&value.to_string());
        first = false;
    }
    printed.push(')');
    printed
}

#[test]
fn allocating_p1_lib_builtins_survive_gc_stress_on_long_lists() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    // `*VEC-TYPE*` holds the already-interned `VECTOR` symbol so the later
    // `COERCE` probe reads a variable instead of compiling a fresh `'VECTOR`
    // quote form. Compiling a new quoted-symbol literal for the first time
    // while a `dotimes`/`setq` loop is also being compiled under `gc_stress`
    // hits a separate, pre-existing front-end defect (`ThreadNotRegistered`
    // from a GC during literal interning) unrelated to `COERCE`'s own
    // implementation; see the "found outside scope" note in the lane report.
    runtime
        .compile("(defparameter *vec-type* 'vector)")
        .unwrap_or_else(|error| panic!("setup compile failed: {error:?}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);

    // Built from `CONS` rather than nested `LIST` calls: nesting a variadic
    // `LIST` call inside another is a separate, pre-existing front-end/codegen
    // defect that fails to compile at all under `gc_stress`.
    assert_eval_with_stress_on(
        &mut runtime,
        "(copy-tree (cons (cons 1 (cons 2 nil)) (cons (cons 3 (cons (cons 4 (cons 5 nil)) nil)) nil)))",
        "((1 2) (3 (4 5)))",
    );

    let long_list = range_list_source();

    assert_eval_with_stress_on(
        &mut runtime,
        &format!("(copy-tree {long_list})"),
        &printed_list(1..=LONG_LIST_LEN),
    );

    assert_eval_with_stress_on(
        &mut runtime,
        &format!("(remove-if #'evenp {long_list})"),
        &printed_list((1..=LONG_LIST_LEN).filter(|value| value % 2 == 1)),
    );

    assert_eval_with_stress_on(
        &mut runtime,
        &format!("(coerce {long_list} *vec-type*)"),
        &printed_vector(1..=LONG_LIST_LEN),
    );

    assert_eval_with_stress_on(
        &mut runtime,
        &format!("(make-array {LONG_LIST_LEN} :initial-contents {long_list})"),
        &printed_vector(1..=LONG_LIST_LEN),
    );
}

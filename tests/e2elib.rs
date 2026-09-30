//! CLI regression coverage for `ncl-types` and `ncl-lib-hash-arrays` builtins
//! that were previously interned but unbound: `COERCE`, `KEYWORDP`,
//! `TYPE-OF`, `SUBTYPEP`, and `MAKE-ARRAY`'s `:initial-contents`.

#![allow(missing_docs)]

use std::process::Command;

fn eval(source: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("failed to run {source}: {error}"));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    )
}

#[test]
fn types_and_array_builtins_run_through_compiled_code() {
    let cases = [
        ("(keywordp :foo)", "T"),
        ("(keywordp 'foo)", "NIL"),
        ("(keywordp 3)", "NIL"),
        ("(type-of 5)", "FIXNUM"),
        ("(type-of \"abc\")", "SIMPLE-STRING"),
        ("(type-of nil)", "NULL"),
        ("(type-of :foo)", "KEYWORD"),
        ("(type-of #(1 2))", "SIMPLE-VECTOR"),
        ("(subtypep 'fixnum 'integer)", "T"),
        ("(subtypep 'integer 'fixnum)", "NIL"),
        ("(subtypep 'string 'integer)", "NIL"),
        ("(subtypep 'cons 'list)", "T"),
        ("(coerce '(1 2 3) 'vector)", "#(1 2 3)"),
        ("(coerce #(1 2 3) 'list)", "(1 2 3)"),
        ("(coerce (/ 1 2) 'float)", "0.5"),
        ("(coerce 3 'float)", "3.0"),
        ("(coerce #\\a 'character)", "#\\a"),
        ("(coerce 5 'integer)", "5"),
        ("(make-array 3 :initial-contents (list 1 2 3))", "#(1 2 3)"),
        (
            "(make-array '(2 2) :initial-contents (list (list 1 2) (list 3 4)))",
            "#2A(1 2 3 4)",
        ),
        (
            "(aref (make-array '(2 2) :initial-contents (list (list 1 2) (list 3 4))) 1 0)",
            "3",
        ),
        (
            "(aref (make-array '(2 2) :initial-contents (list (list 1 2) (list 3 4))) 0 1)",
            "2",
        ),
    ];
    assert_eq!(cases.len(), 22);
    for (source, expected) in cases {
        let (success, actual) = eval(source);
        assert!(success, "{source} failed");
        assert_eq!(actual, expected, "{source}");
    }
}

#[test]
fn make_array_rejects_conflicting_initial_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args([
            "--eval",
            "(make-array 2 :initial-element 0 :initial-contents (list 1 2))",
        ])
        .output()
        .unwrap_or_else(|error| panic!("failed to run: {error}"));
    assert!(!output.status.success());
}

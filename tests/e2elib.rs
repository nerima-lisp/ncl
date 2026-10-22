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
    let output_text = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    (output.status.success(), output_text.trim().to_owned())
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

#[test]
fn gensym_defaults_to_g_and_uses_a_string_prefix_for_one_call() {
    let source =
        "(let ((*gensym-counter* 12)) (list (gensym) (gensym \"TMP\") (gensym) *gensym-counter*))";
    let (success, actual) = eval(source);
    assert!(success, "{source} failed");
    assert_eq!(actual, "(#:G12 #:TMP13 #:G14 15)", "{source}");
}

#[test]
fn gensym_counter_accepts_a_bignum_and_increments_it_exactly() {
    let source = r"(list
        (let ((*gensym-counter* 18446744073709551616))
          (list (symbol-name (gensym)) *gensym-counter*))
        (let ((*gensym-counter* 18446744073709551615))
          (list (symbol-name (gensym)) *gensym-counter*)))";
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(
        actual,
        "((\"G18446744073709551616\" 18446744073709551617) (\"G18446744073709551615\" 18446744073709551616))",
        "{source}"
    );
}

#[test]
fn gensym_counter_promotes_past_the_largest_fixnum() {
    let source = "(let ((*gensym-counter* 4611686018427387903)) (list (symbol-name (gensym)) *gensym-counter*))";
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(
        actual, "(\"G4611686018427387903\" 4611686018427387904)",
        "{source}"
    );
}

#[test]
fn gensym_counter_rejects_negative_and_noninteger_values() {
    for source in [
        "(let ((*gensym-counter* -18446744073709551616)) (gensym))",
        "(let ((*gensym-counter* 'foo)) (gensym))",
    ] {
        let (success, actual) = eval(source);
        assert!(
            !success && actual.contains("TypeError"),
            "{source} did not reject the invalid counter (success={success}, output={actual})"
        );
    }
}

#[test]
fn gensym_integer_suffix_is_fresh_and_does_not_advance_the_counter() {
    let source = "(let ((*gensym-counter* 70)) (let ((first (gensym 5)) (second (gensym 5)) (large (gensym 18446744073709551616)) (next (gensym))) (and (not (eq first second)) (null (symbol-package first)) (null (symbol-package second)) (equal (symbol-name first) \"G5\") (equal (symbol-name large) \"G18446744073709551616\") (equal (symbol-name next) \"G70\") (= *gensym-counter* 71))))";
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(actual, "T", "{source}");
}

#[test]
fn gensym_rejects_arguments_outside_strings_and_nonnegative_integers() {
    for source in [
        "(gensym 'foo)",
        "(gensym #\\a)",
        "(gensym -1)",
        "(gensym -18446744073709551616)",
        "(gensym 1.5)",
    ] {
        let (success, actual) = eval(source);
        assert!(
            !success && actual.contains("TypeError"),
            "{source} did not signal a type error (success={success}, output={actual})"
        );
    }
}

#[test]
fn find_package_accepts_package_objects_and_string_designators() {
    let source = "(let ((package (make-package \"OO15-FIND-PACKAGE\" :use nil))) (and (eq (find-package package) package) (eq (find-package \"OO15-FIND-PACKAGE\") package) (eq (find-package 'oo15-find-package) package) (null (find-package \"OO15-NOT-FOUND\"))))";
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(actual, "T", "{source}");
}

#[test]
fn find_package_rejects_values_outside_package_and_string_designators() {
    for source in ["(find-package 1)", "(find-package #\\a)"] {
        let (success, actual) = eval(source);
        assert!(
            !success && actual.contains("TypeError"),
            "{source} did not reject the invalid name (success={success}, output={actual})"
        );
    }
}

#[test]
fn find_symbol_and_intern_report_package_accessibility_statuses() {
    let source = r#"
        (let* ((package (make-package "OO15-STATUS-PKG" :use nil))
               (created (let ((*package* package))
                          (multiple-value-list (intern "LOCAL"))))
               (found (multiple-value-list (find-symbol "LOCAL" package)))
               (repeated (multiple-value-list (intern "LOCAL" package)))
               (missing (multiple-value-list (find-symbol "MISSING" package)))
               (keyword-created (multiple-value-list (intern "OO15-KEYWORD" "KEYWORD")))
               (keyword-repeat (multiple-value-list (intern "OO15-KEYWORD" "KEYWORD")))
               (keyword-found (multiple-value-list (find-symbol "OO15-KEYWORD" "KEYWORD"))))
          (export (list (first created)) package)
          (let ((external-find (multiple-value-list (find-symbol "LOCAL" package)))
                (external-intern (multiple-value-list (intern "LOCAL" package)))
                (inherited-find (multiple-value-list (find-symbol "CAR" 'common-lisp-user)))
                (inherited-intern (multiple-value-list (intern "CAR" 'common-lisp-user))))
            (list (eq (second created) nil)
                  (eq (first created) (first found))
                  (eq (second found) :internal)
                  (eq (first repeated) (first created))
                  (eq (second repeated) :internal)
                  (null (first missing))
                  (null (second missing))
                  (eq (second keyword-created) nil)
                  (eq (first keyword-repeat) (first keyword-created))
                  (eq (second keyword-repeat) :external)
                  (eq (first keyword-found) (first keyword-created))
                  (eq (second keyword-found) :external)
                  (eq (symbol-value (first keyword-created)) (first keyword-created))
                  (eq (second external-find) :external)
                  (eq (second external-intern) :external)
                  (eq (first inherited-find) 'car)
                  (eq (second inherited-find) :inherited)
                  (eq (first inherited-intern) 'car)
                  (eq (second inherited-intern) :inherited))))
    "#;
    let (success, actual) = eval(source);
    assert!(success, "{source} failed: {actual}");
    assert_eq!(
        actual, "(T T T T T T T T T T T T T T T T T T T)",
        "{source}"
    );
}

#[test]
fn find_symbol_and_intern_reject_non_string_designator_names() {
    for source in [
        "(find-symbol 1)",
        "(find-symbol (find-package \"COMMON-LISP\"))",
        "(intern 1)",
        "(intern (find-package \"COMMON-LISP\"))",
    ] {
        let (success, actual) = eval(source);
        assert!(
            !success && actual.contains("TypeError"),
            "{source} did not reject the invalid name (success={success}, output={actual})"
        );
    }
}

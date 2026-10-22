#![allow(missing_docs)]

use std::process::{Command, Output};

fn eval(source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("failed to run {source}: {error}"))
}

fn assert_eval(source: &str, expected: &str) {
    let output = eval(source);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{source} failed with status {:?}: {stderr}",
        output.status.code()
    );
    assert!(stderr.is_empty(), "{source} wrote to stderr: {stderr}");
    assert_eq!(stdout.trim(), expected, "{source}");
}

fn assert_type_error(source: &str) {
    let output = eval(source);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{source} should fail with a type error (stderr: {stderr})"
    );
    assert_eq!(stderr.trim(), "ncl: object error: TypeError", "{source}");
}

#[test]
fn make_hash_table_accepts_function_objects_for_standard_tests() {
    assert_eval(
        "(list (hash-table-test (make-hash-table :test #'eq)) (hash-table-test (make-hash-table :test #'eql)) (hash-table-test (make-hash-table :test #'equal)) (hash-table-test (make-hash-table :test #'equalp)) (hash-table-test (make-hash-table :test (symbol-function 'eq))))",
        "(EQ EQL EQUAL EQUALP EQ)",
    );
}

#[test]
fn make_hash_table_rejects_invalid_test_designators() {
    assert_type_error("(make-hash-table :test 1)");
    assert_type_error("(make-hash-table :test #'car)");
    assert_type_error("(make-hash-table :test (symbol-function 'car))");
    assert_type_error("(make-hash-table :test #'(lambda (left right) t))");
    assert_type_error(
        "(progn (defun oo23-hash-test (left right) (eq left right)) (make-hash-table :test (symbol-function 'oo23-hash-test)))",
    );
}

#[test]
fn make_hash_table_accepts_each_weakness_option_and_keeps_tables_usable() {
    assert_eval(
        "(list (let ((table (make-hash-table :weakness nil))) (setf (gethash :key table) 99) (list (hash-table-p table) (hash-table-count table) (gethash :key table))) (let ((table (make-hash-table :weakness :key))) (setf (gethash :key table) 99) (list (hash-table-p table) (hash-table-count table) (gethash :key table))) (let ((table (make-hash-table :weakness :value))) (setf (gethash :key table) 99) (list (hash-table-p table) (hash-table-count table) (gethash :key table))) (let ((table (make-hash-table :weakness :key-and-value))) (setf (gethash :key table) 99) (list (hash-table-p table) (hash-table-count table) (gethash :key table))) (let ((table (make-hash-table :weakness :key-or-value))) (setf (gethash :key table) 99) (list (hash-table-p table) (hash-table-count table) (gethash :key table))))",
        "((T 1 99) (T 1 99) (T 1 99) (T 1 99) (T 1 99))",
    );
}

#[test]
fn make_hash_table_rejects_unknown_weakness_options() {
    assert_type_error("(make-hash-table :weakness :both)");
    assert_type_error("(make-hash-table :weakness 1)");
}

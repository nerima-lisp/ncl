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
fn make_array_accepts_mod_element_types_from_the_clhs_example() {
    let cases = [
        (
            "(array-element-type (make-array 1 :element-type (quote (mod 1))))",
            "BIT",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (mod 2))))",
            "BIT",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (mod 16))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(row-major-aref (make-array 1 :element-type (quote (mod 16)) :initial-element 15) 0)",
            "15",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (mod 18446744073709551616))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(row-major-aref (make-array 1 :element-type (quote (mod 18446744073709551616)) :initial-element 18446744073709551615) 0)",
            "18446744073709551615",
        ),
        (
            "(row-major-aref (make-array 2 :element-type (quote (mod 16)) :initial-contents (quote (0 15))) 1)",
            "15",
        ),
        (
            "(let ((array (make-array 1 :element-type (quote (mod 16)) :initial-element 15))) (setf (aref array 0) 16) (aref array 0))",
            "16",
        ),
    ];
    assert_eq!(cases.len(), 8);
    for (source, expected) in cases {
        assert_eval(source, expected);
    }
}

#[test]
fn make_array_accepts_integer_interval_designators() {
    let cases = [
        (
            "(array-element-type (make-array 1 :element-type (quote (integer))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (signed-byte))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (unsigned-byte))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer 0 *))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer -1 *))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer * *))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer 18446744073709551616 *))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer 0 18446744073709551616))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer (0) *))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer (-1) *))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (integer 0 (10)))))",
            "UNSIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (signed-byte 8))))",
            "SIGNED-BYTE",
        ),
        (
            "(array-element-type (make-array 1 :element-type (quote (unsigned-byte *))))",
            "UNSIGNED-BYTE",
        ),
    ];
    assert_eq!(cases.len(), 13);
    for (source, expected) in cases {
        assert_eval(source, expected);
    }
}

#[test]
fn make_array_rejects_invalid_moduli_and_element_type_shapes() {
    let sources = [
        "(make-array 1 :element-type (quote (mod 0)))",
        "(make-array 1 :element-type (quote (mod -1)))",
        "(make-array 1 :element-type (quote (mod *)))",
        "(make-array 1 :element-type (quote (mod 1.5)))",
        "(make-array 1 :element-type (quote (mod)))",
        "(make-array 1 :element-type (quote (mod 16)) :initial-element 16)",
        "(make-array 1 :element-type (quote (mod 16)) :initial-element -1)",
        "(make-array 2 :element-type (quote (mod 16)) :initial-contents (quote (0 16)))",
        "(make-array 1 :element-type (quote (mod 18446744073709551616)) :initial-element 18446744073709551616)",
        "(make-array 1 :element-type (quote (integer (0 1) *)))",
        "(make-array 1 :element-type (quote (integer not-an-integer *)))",
        "(make-array 1 :element-type (quote (integer 0 1 2)))",
        "(make-array 1 :element-type (quote (unsigned-byte 8 16)))",
        "(make-array 1 :element-type (quote unrecognized-array-element-type))",
        "(make-array 1 :element-type 5)",
    ];
    assert_eq!(sources.len(), 15);
    for source in sources {
        assert_type_error(source);
    }
}

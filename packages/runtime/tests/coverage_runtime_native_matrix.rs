//! Value-based checks for native runtime call paths.

#![allow(clippy::unwrap_used, reason = "tests use fixed runtime inputs")]

use ncl_runtime::{Runtime, RuntimeError};

fn eval(runtime: &mut Runtime, source: &str) -> Result<String, RuntimeError> {
    runtime
        .eval(source)
        .map(|value| runtime.format_result(value))
}

#[test]
fn native_call_shapes_are_table_driven() {
    let cases = [
        (
            "wide generic builtin",
            "(list 1 2 3 4 5 6 7 8 9 10)",
            "(1 2 3 4 5 6 7 8 9 10)",
        ),
        (
            "rest lambda",
            "(funcall (lambda (&rest values) values) 1 2 3 4 5 6)",
            "(1 2 3 4 5 6)",
        ),
        (
            "keyword lambda",
            "(funcall (lambda (&key left right) (list left right)) :right 2 :left 1)",
            "(1 2)",
        ),
        (
            "optional supplied flag",
            "(funcall (lambda (value &optional (fallback 9 supplied)) (list value fallback supplied)) 3 4)",
            "(3 4 T)",
        ),
        (
            "builtin function designator",
            "(funcall #'list 4 5 6)",
            "(4 5 6)",
        ),
        (
            "multiple values",
            "(multiple-value-list (floor 7 2))",
            "(3 1)",
        ),
        (
            "compiled macro",
            "(progn (defmacro runtime-native-duplicate (value) (list 'list value value)) (runtime-native-duplicate 3))",
            "(3 3)",
        ),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (label, source, expected) in cases {
        assert_eq!(eval(&mut runtime, source).unwrap(), expected, "{label}");
    }
}

#[test]
fn control_and_condition_boundaries_are_table_driven() {
    let cases = [
        ("catch without throw", "(catch 'tag 12)", "12"),
        ("throw through catch", "(catch 'tag (throw 'tag 13))", "13"),
        (
            "unwind protect normal result",
            "(unwind-protect 14 15)",
            "14",
        ),
        (
            "dynamic binding",
            "(progv '(runtime-native-matrix) '(17) runtime-native-matrix)",
            "17",
        ),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (label, source, expected) in cases {
        let result = eval(&mut runtime, source);
        match result {
            Ok(value) => assert_eq!(value, expected, "{label}"),
            Err(error) => panic!("{label}: {error:?}"),
        }
    }

    let mut runtime = Runtime::new().unwrap();
    let unhandled = eval(&mut runtime, "(catch 'other (throw 'missing 1))");
    assert!(matches!(
        unhandled,
        Err(RuntimeError::Object(ncl_object::ObjectError::ControlError))
    ));
}

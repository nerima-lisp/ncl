#![allow(clippy::unwrap_used, reason = "tests use fixed runtime inputs")]

use ncl_runtime::{Runtime, RuntimeError};

#[test]
fn public_runtime_operations_and_result_shapes_are_table_driven() {
    let cases = [
        ("eval", "eval", "21", "21"),
        ("compile", "compile", "22", "22"),
        ("load", "load", "23", "23"),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (label, operation, source, expected) in cases {
        let value = match operation {
            "eval" => runtime.eval(source).unwrap(),
            "compile" => runtime.compile(source).unwrap(),
            "load" => runtime.load(source).unwrap(),
            _ => panic!("unknown operation: {operation}"),
        };
        assert_eq!(runtime.format_result(value), expected, "{label}");
    }

    let result_cases = [
        ("true", "t", "T"),
        ("nil", "nil", "NIL"),
        ("string", "\"runtime\"", "\"runtime\""),
        ("list", "(list 1 2)", "(1 2)"),
    ];
    for (label, source, expected) in result_cases {
        let value = runtime.eval(source).unwrap();
        assert_eq!(runtime.format_result(value), expected, "{label}");
    }
}

#[test]
fn public_runtime_errors_preserve_their_value_categories() {
    let mut runtime = Runtime::new().unwrap();
    let cases = [
        ("reader", "(", "reader"),
        (
            "undefined function",
            "(runtime-api-matrix-missing)",
            "undefined",
        ),
    ];

    for (label, source, kind) in cases {
        let error = runtime.eval(source).expect_err(label);
        match kind {
            "reader" => assert!(
                matches!(&error, RuntimeError::Read(_)),
                "{label}: {error:?}"
            ),
            "undefined" => assert!(
                matches!(
                    &error,
                    RuntimeError::UndefinedFunction { name }
                        if name == "RUNTIME-API-MATRIX-MISSING"
                ),
                "{label}: {error:?}"
            ),
            _ => panic!("unknown error category: {kind}"),
        }
    }
}

#[test]
fn runtime_modes_can_be_toggled_before_a_value_based_probe() {
    let mut runtime = Runtime::new().unwrap();
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    let value = runtime.eval("(+ 19 23)").unwrap();
    assert_eq!(runtime.format_result(value), "42");
    runtime.set_gc_stress(false);
    runtime.set_strict_forwarding(false);
    let value = runtime.eval("(+ 20 22)").unwrap();
    assert_eq!(runtime.format_result(value), "42");
}

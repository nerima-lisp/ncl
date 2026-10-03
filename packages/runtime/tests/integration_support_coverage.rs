//! Public-API coverage for compiled constant materialization.

#![allow(clippy::unwrap_used, reason = "tests use fixed runtime inputs")]

use ncl_runtime::{Runtime, RuntimeError};

fn compile_result(runtime: &mut Runtime, source: &str) -> String {
    let value = runtime.compile(source).unwrap();
    runtime.format_result(value)
}

#[test]
fn compile_materializes_constant_shapes_with_exact_values() {
    let cases = [
        ("42", "42"),
        (
            "'runtime-support-symbol",
            "COMMON-LISP-USER:RUNTIME-SUPPORT-SYMBOL",
        ),
        ("\"runtime support\"", "\"runtime support\""),
        ("#(1 2 3)", "#(1 2 3)"),
        ("'(1 . 2)", "(1 . 2)"),
        ("1.5", "1.5"),
        ("4611686018427387904", "4611686018427387904"),
        ("2/3", "2/3"),
        ("#C(1 2)", "#C(1 2)"),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (source, expected) in cases {
        assert_eq!(compile_result(&mut runtime, source), expected, "{source}");
    }
}

#[test]
fn compile_reports_exact_reader_and_front_end_errors() {
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.compile("("),
        Err(RuntimeError::Read(ncl_reader::ReadError::UnexpectedEof))
    ));
    assert!(matches!(
        runtime.compile("(if 1)"),
        Err(RuntimeError::Front(
            ncl_compiler_front::FrontError::WrongNumberOfForms {
                operator: _,
                expected: "a test and a then form",
                found: 1,
            }
        ))
    ));
    assert!(matches!(
        runtime.compile("(integration-support-coverage-missing)"),
        Err(RuntimeError::UndefinedFunction { name })
            if name == "INTEGRATION-SUPPORT-COVERAGE-MISSING"
    ));
}

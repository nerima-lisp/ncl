#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests use fixed runtime inputs"
)]

use std::fs;

use ncl_runtime::{Runtime, RuntimeError};

fn eval(runtime: &mut Runtime, source: &str) -> Result<String, RuntimeError> {
    runtime
        .eval(source)
        .map(|value| runtime.format_result(value))
}

fn assert_type_error(result: Result<String, RuntimeError>) {
    let error = result.err().unwrap();
    assert!(matches!(
        error,
        RuntimeError::Object(ncl_object::ObjectError::TypeError)
    ));
    assert_eq!(error.to_string(), "object error: TypeError");
}

#[test]
fn function_call_public_paths_return_values_and_report_bad_designators() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(eval(&mut runtime, "(funcall #'+ 20 22)").unwrap(), "42");
    assert_eq!(eval(&mut runtime, "(apply #'+ '(20 22))").unwrap(), "42");
    assert_type_error(eval(&mut runtime, "(funcall 42)"));

    let error = runtime
        .eval("(funcall 'integration-runtime-missing 1)")
        .err()
        .unwrap();
    assert!(matches!(
        error,
        RuntimeError::UndefinedFunction { ref name }
            if name == "INTEGRATION-RUNTIME-MISSING"
    ));
    assert_eq!(
        error.to_string(),
        "undefined function UNDEFINED-FUNCTION: INTEGRATION-RUNTIME-MISSING"
    );
}

#[test]
fn public_file_apis_round_trip_source_and_fasl_results() {
    let stem = std::env::temp_dir().join(format!(
        "ncl-runtime-integration-edges-{}",
        std::process::id()
    ));
    let source = stem.with_extension("lisp");
    let fasl = stem.with_extension("fasl");
    fs::write(&source, "(+ 20 22)").unwrap();
    let mut runtime = Runtime::new().unwrap();

    let compiled = runtime.compile_file(&source).unwrap();
    assert_eq!(runtime.format_result(compiled), "42");
    let loaded = runtime.load_file(&fasl).unwrap();
    assert_eq!(runtime.format_result(loaded), "42");

    let missing = stem.with_extension("missing");
    let error = runtime.load_file(&missing).err().unwrap();
    assert!(matches!(
        error,
        RuntimeError::Io { ref path, .. } if path == missing.to_string_lossy().as_ref()
    ));
    assert_eq!(
        error.to_string(),
        format!(
            "cannot read {}: No such file or directory (os error 2)",
            missing.display()
        )
    );

    fs::remove_file(&source).unwrap();
    fs::remove_file(&fasl).unwrap();
}

#[test]
fn nonlocal_public_paths_preserve_values_and_control_error_text() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(catch 'integration-tag (unwind-protect (throw 'integration-tag 7) 8))",
        )
        .unwrap(),
        "7"
    );

    let error = runtime
        .eval("(catch 'integration-other (throw 'integration-missing 1))")
        .err()
        .unwrap();
    assert!(matches!(
        error,
        RuntimeError::Object(ncl_object::ObjectError::ControlError)
    ));
    assert_eq!(error.to_string(), "object error: ControlError");
}

#[test]
fn arithmetic_public_paths_return_exact_numbers_and_classify_failures() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(eval(&mut runtime, "(/ 7 2)").unwrap(), "7/2");
    assert_eq!(
        eval(&mut runtime, "(* 4611686018427387903 2)").unwrap(),
        "9223372036854775806"
    );
    assert_type_error(eval(&mut runtime, "(/ 8 0)"));
    assert_type_error(eval(&mut runtime, "(+ 1 'not-a-number)"));
}

//! Public-API integration checks for the generic builtin trampoline.

#![allow(clippy::unwrap_used, reason = "tests use fixed runtime inputs")]

use std::fs;

use ncl_runtime::{Runtime, RuntimeError};

fn eval(runtime: &mut Runtime, source: &str) -> Result<String, RuntimeError> {
    runtime
        .eval(source)
        .map(|value| runtime.format_result(value))
}

#[test]
fn builtin_trampoline_reports_arity_and_type_errors() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(handler-case (car) (error (condition) :wrong-arity))",
        )
        .unwrap(),
        ":WRONG-ARITY"
    );
    assert!(matches!(
        runtime.eval("(car 7)"),
        Err(RuntimeError::NativeFailure {
            condition: ncl_runtime::NativeCondition::Lisp(ncl_object::LispError::TypeError { .. }),
            ..
        })
    ));
}

#[test]
fn builtin_trampoline_forwards_rest_arguments_beyond_registers() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(&mut runtime, "(list 1 2 3 4 5 6 7 8 9 10)").unwrap(),
        "(1 2 3 4 5 6 7 8 9 10)"
    );
}

#[test]
fn builtin_trampoline_handles_keyword_missing_unknown_and_supplied_cases() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(funcall (lambda (&key (value 7 supplied)) (list value supplied)))",
        )
        .unwrap(),
        "(7 NIL)"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(funcall (lambda (&key (value 7 supplied)) (list value supplied)) :value 9)",
        )
        .unwrap(),
        "(9 T)"
    );
    assert!(matches!(
        runtime.eval("(funcall (lambda (&key value) value) :other 9)"),
        Err(RuntimeError::Object(ncl_object::ObjectError::ControlError))
    ));
}

#[test]
fn builtin_trampoline_preserves_multiple_values() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(&mut runtime, "(multiple-value-list (floor 17 5))").unwrap(),
        "(3 2)"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(multiple-value-call #'list (values :first :second :third))",
        )
        .unwrap(),
        "(:FIRST :SECOND :THIRD)"
    );
}

#[test]
fn builtin_trampoline_reports_undefined_functions() {
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.eval("(integration-trampoline-missing-function)"),
        Err(RuntimeError::UndefinedFunction { name })
            if name == "INTEGRATION-TRAMPOLINE-MISSING-FUNCTION"
    ));
}

#[test]
fn public_load_and_eval_boundaries_preserve_runtime_state() {
    let path = std::env::temp_dir().join(format!(
        "ncl-runtime-trampoline-boundary-{}.lisp",
        std::process::id()
    ));
    fs::write(
        &path,
        "(defun integration-trampoline-loaded (value) (list :loaded value))",
    )
    .unwrap();
    let path_string = path.to_string_lossy().replace('"', "\\\"");
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        runtime
            .load("(+ 20 22)")
            .map(|value| runtime.format_result(value))
            .unwrap(),
        "42"
    );
    assert_eq!(
        eval(&mut runtime, &format!("(load \"{path_string}\")"),).unwrap(),
        "COMMON-LISP-USER:INTEGRATION-TRAMPOLINE-LOADED"
    );
    assert_eq!(
        eval(&mut runtime, "(integration-trampoline-loaded 5)").unwrap(),
        "(:LOADED 5)"
    );

    fs::remove_file(path).unwrap();
}

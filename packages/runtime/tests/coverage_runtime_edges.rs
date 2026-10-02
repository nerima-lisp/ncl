#![allow(clippy::unwrap_used, missing_docs)]

use std::fs;

use ncl_runtime::{Runtime, RuntimeError};

fn eval(runtime: &mut Runtime, source: &str) -> String {
    let value = runtime.eval(source).unwrap();
    runtime.format_result(value)
}

#[test]
fn top_level_wrappers_return_the_last_form() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(eval(&mut runtime, "(progn 1 2 3)"), "3");
    assert_eq!(
        eval(&mut runtime, "(locally (declare (special *x*)) 4 5)"),
        "5"
    );
    assert_eq!(
        eval(&mut runtime, "(macrolet ((twice (x) (+ x x))) (twice 6))"),
        "12"
    );
}

#[test]
fn local_macro_expander_calls_a_compiled_function() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(progn (defun increment (x) (+ x 1))\
                    (macrolet ((twice (x) (increment (increment x))))\
                      (twice 5)))",
        ),
        "7"
    );
}

#[test]
fn compile_file_rejects_non_utf8_source() {
    let path = std::env::temp_dir().join(format!(
        "ncl-runtime-invalid-source-{}.lisp",
        std::process::id()
    ));
    fs::write(&path, [0xff]).unwrap();
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.compile_file(&path),
        Err(RuntimeError::Native(message)) if message.contains("not valid UTF-8")
    ));
    fs::remove_file(path).unwrap();
}

#[test]
fn eval_when_selects_execute_and_rejects_malformed_situations() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(eval(&mut runtime, "(eval-when (:execute) 7)"), "7");
    assert_eq!(
        eval(&mut runtime, "(eval-when (:compile-toplevel) 8)"),
        "NIL"
    );
    assert!(matches!(
        runtime.eval("(eval-when (:not-a-situation) 9)"),
        Err(RuntimeError::Front(_))
    ));
    assert!(matches!(
        runtime.eval("(eval-when)"),
        Err(RuntimeError::Front(_))
    ));
}

#[test]
fn eval_when_accepts_nested_situations_and_skips_compile_only_forms() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(progn (eval-when ((:execute :eval) 11) 12) 13)",
        ),
        "13"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(eval-when (:compile-toplevel :load-toplevel) 14)",
        ),
        "NIL"
    );
    assert!(matches!(
        runtime.eval("(eval-when (42) 15)"),
        Err(RuntimeError::Front(_))
    ));
}

#[test]
fn evaluation_reports_undefined_functions_and_reader_errors() {
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.eval("(runtime-function-that-does-not-exist 1)"),
        Err(RuntimeError::UndefinedFunction { name }) if name == "RUNTIME-FUNCTION-THAT-DOES-NOT-EXIST"
    ));
    assert!(matches!(runtime.eval("(if"), Err(RuntimeError::Read(_))));
}

#[test]
fn non_local_control_restores_bindings_and_runs_cleanup() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(
            &mut runtime,
            "(catch 'cleanup\
                (catch 'tag\
                  (unwind-protect (throw 'tag 42) (throw 'cleanup 99))))",
        ),
        "99"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(progv '(runtime-special) '(17) runtime-special)",
        ),
        "17"
    );
    assert!(matches!(
        runtime.eval("(throw 'missing 1)"),
        Err(RuntimeError::Object(ncl_object::ObjectError::NonLocalExit))
    ));
}

#[test]
fn multiple_values_are_forwarded_to_a_consumer() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(&mut runtime, "(multiple-value-call #'list (values 1 2 3))",),
        "(1 2 3)"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(multiple-value-call #'list\
                (multiple-value-prog1 (values 4 5) 6))",
        ),
        "(4 5)"
    );
}

#[test]
fn arithmetic_overflow_uses_the_numeric_tower_fallback() {
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(&mut runtime, "(+ 4611686018427387903 1)"),
        "4611686018427387904"
    );
}

#[test]
fn load_builtin_executes_file_and_honors_if_does_not_exist() {
    let path =
        std::env::temp_dir().join(format!("ncl-runtime-load-edge-{}.lisp", std::process::id()));
    fs::write(&path, "(+ 20 22)").unwrap();
    let path_string = path.to_string_lossy().replace('"', "\\\"");
    let missing = path.with_extension("missing");
    let missing_string = missing.to_string_lossy().replace('"', "\\\"");
    let mut runtime = Runtime::new().unwrap();

    assert_eq!(
        eval(&mut runtime, &format!("(load \"{path_string}\")")),
        "42"
    );
    assert_eq!(
        eval(
            &mut runtime,
            &format!("(load \"{missing_string}\" :if-does-not-exist nil)"),
        ),
        "NIL"
    );
    let missing_result = runtime.eval(&format!("(load \"{missing_string}\")"));
    assert!(matches!(
        missing_result,
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert!(path.is_file());
    fs::remove_file(&path).unwrap();
    assert!(!path.exists());
}

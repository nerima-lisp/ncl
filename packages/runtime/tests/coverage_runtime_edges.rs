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
fn evaluation_reports_undefined_functions_and_reader_errors() {
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.eval("(runtime-function-that-does-not-exist 1)"),
        Err(RuntimeError::UndefinedFunction { name }) if name == "RUNTIME-FUNCTION-THAT-DOES-NOT-EXIST"
    ));
    assert!(matches!(runtime.eval("(if"), Err(RuntimeError::Read(_))));
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

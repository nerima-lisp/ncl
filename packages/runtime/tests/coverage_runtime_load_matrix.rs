#![allow(clippy::unwrap_used, reason = "tests use fixed temporary inputs")]

use std::fs;

use ncl_runtime::{Runtime, RuntimeError};

fn eval(runtime: &mut Runtime, source: &str) -> Result<String, RuntimeError> {
    runtime
        .eval(source)
        .map(|value| runtime.format_result(value))
}

fn quoted(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}

#[test]
fn top_level_loader_forms_are_table_driven() {
    let cases = [
        ("plain form", "(+ 2 3)", "5"),
        ("empty progn", "(progn)", "NIL"),
        ("empty locally", "(locally)", "NIL"),
        (
            "symbol macrolet",
            "(symbol-macrolet ((answer 42)) answer)",
            "42",
        ),
        ("execute eval-when", "(eval-when (:execute) 7)", "7"),
        (
            "load eval-when",
            "(eval-when (:compile-toplevel :load-toplevel) 8)",
            "8",
        ),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (label, source, expected) in cases {
        assert_eq!(eval(&mut runtime, source).unwrap(), expected, "{label}");
    }
}

#[test]
fn load_options_and_file_failures_are_table_driven() {
    let valid_path = std::env::temp_dir().join(format!(
        "ncl-runtime-load-matrix-valid-{}.lisp",
        std::process::id()
    ));
    let invalid_path = std::env::temp_dir().join(format!(
        "ncl-runtime-load-matrix-invalid-{}.lisp",
        std::process::id()
    ));
    let utf8_path = std::env::temp_dir().join(format!(
        "ncl-runtime-load-matrix-utf8-{}.lisp",
        std::process::id()
    ));
    fs::write(&valid_path, "(+ 20 22)").unwrap();
    fs::write(&invalid_path, "(runtime-load-matrix-missing)").unwrap();
    fs::write(&utf8_path, [0xff]).unwrap();
    let valid = quoted(&valid_path);
    let invalid = quoted(&invalid_path);
    let utf8 = quoted(&utf8_path);
    let missing = quoted(&valid_path.with_extension("missing"));
    let cases = [
        (
            "accepted options",
            format!("(load \"{valid}\" :verbose nil :print t :external-format :default)"),
            "42",
        ),
        (
            "missing file suppressed",
            format!("(load \"{missing}\" :if-does-not-exist nil)"),
            "NIL",
        ),
    ];
    let mut runtime = Runtime::new().unwrap();

    for (label, source, expected) in cases {
        assert_eq!(eval(&mut runtime, &source).unwrap(), expected, "{label}");
    }

    let invalid_result = eval(&mut runtime, &format!("(load \"{invalid}\")"));
    assert!(matches!(
        invalid_result,
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    let utf8_result = eval(&mut runtime, &format!("(load \"{utf8}\")"));
    assert!(matches!(
        utf8_result,
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));

    for path in [&valid_path, &invalid_path, &utf8_path] {
        fs::remove_file(path).unwrap();
    }
}

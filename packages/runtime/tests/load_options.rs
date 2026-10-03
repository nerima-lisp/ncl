//! Option and top-level boundary coverage for the runtime loader.

#![allow(clippy::unwrap_used, reason = "tests assert on loader results")]

use std::fs;

use ncl_runtime::{Runtime, RuntimeError};

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ncl-runtime-{name}-{}", std::process::id()))
}

fn quoted(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}

#[test]
fn load_accepts_supported_options_and_rejects_unknown_keywords() {
    let file = path("load-options.lisp");
    fs::write(&file, "(+ 30 12)").unwrap();
    let name = quoted(&file);
    let mut runtime = Runtime::new().unwrap();

    let value = runtime
        .eval(&format!(
            "(load \"{name}\" :verbose t :print nil :external-format :default)"
        ))
        .unwrap();
    assert_eq!(runtime.format_result(value), "42");
    assert!(matches!(
        runtime.eval(&format!("(load \"{name}\" :not-an-option t)")),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    fs::remove_file(file).unwrap();
}

#[test]
fn load_rejects_non_default_external_formats_and_malformed_option_pairs() {
    let file = path("load-format.lisp");
    fs::write(&file, "1").unwrap();
    let name = quoted(&file);
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.eval(&format!("(load \"{name}\" :external-format :utf-8)")),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert!(matches!(
        runtime.eval(&format!("(load \"{name}\" :verbose)")),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    fs::remove_file(file).unwrap();
}

#[test]
fn load_validates_path_and_option_arguments_before_opening_a_file() {
    let mut runtime = Runtime::new().unwrap();

    assert!(matches!(
        runtime.eval("(load)"),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert!(matches!(
        runtime.eval("(load 42)"),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
    assert!(matches!(
        runtime.eval("(load \"missing\" :if-does-not-exist t)"),
        Err(RuntimeError::Object(ncl_object::ObjectError::TypeError))
    ));
}

#[test]
fn top_level_package_forms_update_the_reader_package_for_later_forms() {
    let mut runtime = Runtime::new().unwrap();

    let value = runtime
        .load(
            "(in-package :COMMON-LISP-USER)\n\
             99",
        )
        .unwrap();
    assert_eq!(runtime.format_result(value), "99");
}

#[test]
fn top_level_package_string_and_multiple_declarations_preserve_the_last_value() {
    let mut runtime = Runtime::new().unwrap();

    let value = runtime
        .load(
            "(in-package \"COMMON-LISP-USER\")\n\
             (locally\n\
               (declare (special *runtime-load-option-test*))\n\
               (declare (optimize (speed 3)))\n\
               101\n\
               102)",
        )
        .unwrap();

    assert_eq!(runtime.format_result(value), "102");
}

#[test]
fn load_file_rejects_non_utf8_source_with_a_specific_native_error() {
    let file = path("invalid-utf8");
    fs::write(&file, [0xff]).unwrap();
    let mut runtime = Runtime::new().unwrap();

    let result = runtime.load_file(&file);

    assert!(matches!(
        result,
        Err(RuntimeError::Native(message))
            if message == "source file is not valid UTF-8"
    ));
    fs::remove_file(file).unwrap();
}

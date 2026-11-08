//! Failure-path behavior tests for COMPILE-FILE.

use ncl_runtime::{Runtime, RuntimeError};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_source_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system clock before Unix epoch: {error}"))
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ncl-compile-file-{label}-{}-{nonce}.lisp",
        std::process::id()
    ))
}

#[test]
fn compile_file_reports_missing_path_without_changing_file_bindings() {
    let missing = unique_source_path("missing");
    assert!(!missing.exists());
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime
        .eval(
            "(setq *compile-file-pathname* :outside-pathname \
                  *compile-file-truename* :outside-truename)",
        )
        .unwrap_or_else(|error| panic!("compile-file binding setup: {error:?}"));

    let result = runtime.compile_file(&missing);

    assert!(matches!(
        result,
        Err(RuntimeError::Io { path, error })
            if path == missing.display().to_string()
                && error.kind() == std::io::ErrorKind::NotFound
    ));
    let bindings_restored = runtime
        .eval(
            "(and (eq *compile-file-pathname* :outside-pathname) \
                  (eq *compile-file-truename* :outside-truename))",
        )
        .unwrap_or_else(|error| panic!("compile-file binding check: {error:?}"));
    assert_eq!(runtime.format_result(bindings_restored), "T");
}

#[test]
fn compile_file_read_error_restores_bindings_without_writing_fasl() {
    let source = unique_source_path("reader-error");
    let fasl = source.with_extension("fasl");
    std::fs::write(&source, "(")
        .unwrap_or_else(|error| panic!("incomplete compile-file source creation: {error}"));
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime
        .eval(
            "(setq *compile-file-pathname* :outside-pathname \
                  *compile-file-truename* :outside-truename)",
        )
        .unwrap_or_else(|error| panic!("compile-file binding setup: {error:?}"));

    let result = runtime.compile_file(&source);
    let fasl_was_written = fasl.exists();
    let cleanup = std::fs::remove_file(&source);

    // CLHS READ specifies an end-of-file error when input ends inside an object, such as an unclosed list.
    // https://www.lispworks.com/documentation/HyperSpec/Body/f_rd_rd.htm
    assert!(matches!(result, Err(error) if error.is_incomplete_read()));
    assert!(!fasl_was_written);
    cleanup.unwrap_or_else(|error| panic!("incomplete source cleanup: {error}"));
    let bindings_restored = runtime
        .eval(
            "(and (eq *compile-file-pathname* :outside-pathname) \
                  (eq *compile-file-truename* :outside-truename))",
        )
        .unwrap_or_else(|error| panic!("compile-file binding check: {error:?}"));
    assert_eq!(runtime.format_result(bindings_restored), "T");
}

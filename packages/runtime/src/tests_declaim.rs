use crate::Runtime;
use std::{fs, path::Path};

#[test]
fn compile_file_applies_declaim_and_proclaim_special_at_compile_time() {
    let source = std::env::temp_dir().join(format!("ncl-declaim-{}.lisp", std::process::id()));
    let output = source.with_extension("fasl");
    let forms = "(declaim (ftype (function (t) t) ncl-declaim-reader)) \
                 (declaim (type list *ncl-declaim-state*)) \
                 (declaim (special *ncl-declaim-state*)) \
                 (defun ncl-declaim-reader () *ncl-declaim-state*)";
    fs::write(&source, forms).unwrap_or_else(|error| panic!("source write failed: {error}"));

    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime setup failed: {error}"));
    runtime
        .compile_file(&source)
        .unwrap_or_else(|error| panic!("COMPILE-FILE with DECLAIM failed: {error}"));
    let result = runtime
        .eval("(let ((*ncl-declaim-state* :bound)) (ncl-declaim-reader))")
        .unwrap_or_else(|error| panic!("proclaimed special variable was not dynamic: {error}"));
    assert_eq!(runtime.format_result(result), ":BOUND");

    for path in [source.as_path(), Path::new(&output)] {
        fs::remove_file(path)
            .unwrap_or_else(|error| panic!("temporary file cleanup failed: {error}"));
    }
}

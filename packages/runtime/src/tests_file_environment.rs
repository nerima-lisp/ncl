use crate::Runtime;
use std::{fs, path::Path};

#[path = "tests_declaim.rs"]
mod tests_declaim;

fn lisp_string(value: &Path) -> String {
    format!("{:?}", value.to_string_lossy())
}

#[test]
fn compile_file_expands_with_open_file_minimal_form() {
    let input = std::env::temp_dir().join(format!(
        "ncl-with-open-file-compile-{}.lisp",
        std::process::id()
    ));
    let source =
        "(defun repro-with-open-file () (with-open-file (stream \"unused-input.txt\") nil))";
    if let Err(error) = fs::write(&input, source) {
        panic!("WITH-OPEN-FILE regression source creation failed: {error}");
    }
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    if let Err(error) = runtime.compile_file(&input) {
        panic!("COMPILE-FILE failed to expand WITH-OPEN-FILE: {error}");
    }
    if let Err(error) = fs::remove_file(&input) {
        panic!("WITH-OPEN-FILE regression source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_file(input.with_extension("fasl")) {
        panic!("WITH-OPEN-FILE regression output cleanup failed: {error}");
    }
}

#[test]
fn compile_file_builtin_compiles_and_loads_fasl_with_three_values() {
    let input =
        std::env::temp_dir().join(format!("ncl-compile-and-load-{}.lisp", std::process::id()));
    if let Err(error) = fs::write(
        &input,
        include_str!("../tests/fixtures/compile-file-source.lisp"),
    ) {
        panic!("COMPILE-FILE source creation failed: {error}");
    }
    let input_truename = match fs::canonicalize(&input) {
        Ok(path) => path,
        Err(error) => panic!("COMPILE-FILE source canonicalization failed: {error}"),
    };
    let output_truename = input_truename.with_extension("fasl");
    let harness = include_str!("../tests/fixtures/compile-and-load.lisp")
        .replace(
            "SOURCE-PATHNAME-HERE",
            &format!("#p{}", lisp_string(&input)),
        )
        .replace("INPUT-NAMESTRING-HERE", &lisp_string(&input))
        .replace(
            "INPUT-TRUENAME-NAMESTRING-HERE",
            &lisp_string(&input_truename),
        )
        .replace(
            "OUTPUT-TRUENAME-NAMESTRING-HERE",
            &lisp_string(&output_truename),
        );
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let result = match runtime.eval(&harness) {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE and LOAD failed: {error}"),
    };
    assert_eq!(runtime.format_result(result), "(T T T T T T T T)");
    let output_bytes = match fs::read(&output_truename) {
        Ok(bytes) => bytes,
        Err(error) => panic!("COMPILE-FILE output read failed: {error}"),
    };
    assert!(output_bytes.starts_with(b"NCLFASL\0"));
    if let Err(error) = fs::remove_file(&input) {
        panic!("COMPILE-FILE source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_file(&output_truename) {
        panic!("COMPILE-FILE output cleanup failed: {error}");
    }
}

#[test]
fn compile_file_binds_pathname_and_truename() {
    let filename = format!("ncl-compile-file-truename-{}.lisp", std::process::id());
    let base = std::env::temp_dir();
    let input = base.join(&filename);
    let input_path = Path::new(&base).join(".").join(&filename);
    if let Err(error) = fs::write(
        &input,
        "(eval-when (:compile-toplevel) \
            (setq *compile-file-pathname-observed* (namestring *compile-file-pathname*) \
                  *compile-file-truename-observed* (namestring *compile-file-truename*)))",
    ) {
        panic!("COMPILE-FILE source creation failed: {error}");
    }
    let truename = match fs::canonicalize(&input) {
        Ok(path) => path,
        Err(error) => panic!("COMPILE-FILE source canonicalization failed: {error}"),
    };
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    if let Err(error) = runtime.eval(
        "(defparameter *compile-file-pathname-observed* nil) \
         (defparameter *compile-file-truename-observed* nil) \
         (setq *compile-file-pathname* :outside-pathname \
               *compile-file-truename* :outside-true)",
    ) {
        panic!("COMPILE-FILE truename setup failed: {error}");
    }
    if let Err(error) = runtime.compile_file(&input_path) {
        panic!("COMPILE-FILE failed: {error}");
    }
    let pathname_observed = match runtime.eval("*compile-file-pathname-observed*") {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE pathname was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(pathname_observed),
        lisp_string(&input_path)
    );
    let truename_observed = match runtime.eval("*compile-file-truename-observed*") {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE truename was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(truename_observed),
        lisp_string(&truename)
    );
    let pathname_restored = match runtime.eval("(eq *compile-file-pathname* :outside-pathname)") {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE pathname was not restored: {error}"),
    };
    assert_eq!(runtime.format_result(pathname_restored), "T");
    let truename_restored = match runtime.eval("(eq *compile-file-truename* :outside-true)") {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE truename was not restored: {error}"),
    };
    assert_eq!(runtime.format_result(truename_restored), "T");
    if let Err(error) = fs::remove_file(&input) {
        panic!("COMPILE-FILE source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_file(input_path.with_extension("fasl")) {
        panic!("COMPILE-FILE output cleanup failed: {error}");
    }
}

#[test]
fn load_binds_load_truename_and_sees_compile_file_truename_bound() {
    let filename = format!("ncl-load-truename-{}.lisp", std::process::id());
    let input = std::env::temp_dir().join(&filename);
    if let Err(error) = fs::write(
        &input,
        "(setq *load-pathname-observed* (namestring *load-pathname*) \
                *load-truename-observed* (namestring *load-truename*) \
                *compile-file-truename-bound* (boundp '*compile-file-truename*))",
    ) {
        panic!("LOAD source creation failed: {error}");
    }
    let truename = match fs::canonicalize(&input) {
        Ok(path) => path,
        Err(error) => panic!("LOAD source canonicalization failed: {error}"),
    };
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    if let Err(error) = runtime.eval(
        "(defparameter *load-pathname-observed* nil) \
         (defparameter *load-truename-observed* nil) \
         (defparameter *compile-file-truename-bound* nil) \
         (setq *load-pathname* :outside-pathname \
               *load-truename* :outside-true)",
    ) {
        panic!("LOAD truename setup failed: {error}");
    }
    if let Err(error) = runtime.load_file(&input) {
        panic!("LOAD failed: {error}");
    }
    let pathname_observed = match runtime.eval("*load-pathname-observed*") {
        Ok(value) => value,
        Err(error) => panic!("LOAD pathname was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(pathname_observed),
        lisp_string(&input)
    );
    let truename_observed = match runtime.eval("*load-truename-observed*") {
        Ok(value) => value,
        Err(error) => panic!("LOAD truename was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(truename_observed),
        lisp_string(&truename)
    );
    let compile_file_truename_bound = match runtime.eval("*compile-file-truename-bound*") {
        Ok(value) => value,
        Err(error) => panic!("COMPILE-FILE-TRUENAME was unbound during LOAD: {error}"),
    };
    assert_eq!(runtime.format_result(compile_file_truename_bound), "T");
    let pathname_restored = match runtime.eval("(eq *load-pathname* :outside-pathname)") {
        Ok(value) => value,
        Err(error) => panic!("LOAD pathname was not restored: {error}"),
    };
    assert_eq!(runtime.format_result(pathname_restored), "T");
    let truename_restored = match runtime.eval("(eq *load-truename* :outside-true)") {
        Ok(value) => value,
        Err(error) => panic!("LOAD truename was not restored: {error}"),
    };
    assert_eq!(runtime.format_result(truename_restored), "T");
    if let Err(error) = fs::remove_file(&input) {
        panic!("LOAD source cleanup failed: {error}");
    }
}

#[test]
fn load_merges_relative_pathname_and_binds_load_pathname_and_truename() {
    let directory =
        std::env::temp_dir().join(format!("ncl-load-relative-path-{}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&directory) {
        panic!("LOAD directory creation failed: {error}");
    }
    let input = directory.join("compile-and-load.lsp");
    if let Err(error) = fs::write(
        &input,
        "(setq *load-pathname-observed* (namestring *load-pathname*) \
                *load-truename-observed* (namestring *load-truename*)) \
         (print \"MINIMAL-LOAD-OK\")",
    ) {
        panic!("LOAD source creation failed: {error}");
    }
    let truename = match fs::canonicalize(&input) {
        Ok(path) => path,
        Err(error) => panic!("LOAD source canonicalization failed: {error}"),
    };
    let default_pathname = Path::new(&format!("{}/", directory.display())).to_path_buf();
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    if let Err(error) = runtime.eval(
        "(defparameter *load-pathname-observed* nil) \
         (defparameter *load-truename-observed* nil) \
         (defparameter *load-outer-truename-observed* nil)",
    ) {
        panic!("LOAD observation setup failed: {error}");
    }
    let form = format!(
        "(progv '(cl:*compile-file-truename* cl:*load-truename*) \
             (list nil #p\"gclload1.lsp\") \
             (let ((*default-pathname-defaults* #p{})) \
               (let ((result (load \"compile-and-load.lsp\"))) \
                 (setq *load-outer-truename-observed* \
                       (namestring *load-truename*)) \
                 result)))",
        lisp_string(&default_pathname)
    );
    let result = match runtime.eval(&form) {
        Ok(value) => value,
        Err(error) => panic!("relative LOAD failed: {error}"),
    };
    assert_eq!(runtime.format_result(result), "\"MINIMAL-LOAD-OK\"");
    let pathname_observed = match runtime.eval("*load-pathname-observed*") {
        Ok(value) => value,
        Err(error) => panic!("LOAD pathname was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(pathname_observed),
        lisp_string(&input)
    );
    let truename_observed = match runtime.eval("*load-truename-observed*") {
        Ok(value) => value,
        Err(error) => panic!("LOAD truename was not observed: {error}"),
    };
    assert_eq!(
        runtime.format_result(truename_observed),
        lisp_string(&truename)
    );
    let outer_truename = match runtime.eval("*load-outer-truename-observed*") {
        Ok(value) => value,
        Err(error) => panic!("outer LOAD truename binding was not observed: {error}"),
    };
    assert_eq!(runtime.format_result(outer_truename), "\"gclload1.lsp\"");
    if let Err(error) = fs::remove_file(&input) {
        panic!("LOAD source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_dir(&directory) {
        panic!("LOAD directory cleanup failed: {error}");
    }
}

#[test]
fn load_missing_file_signals_file_error() {
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let filename = format!("ncl-missing-load-{}.lisp", std::process::id());
    let form = format!(
        "(handler-case (load {}) \
           (error (condition) (typep condition 'file-error)))",
        lisp_string(Path::new(&filename))
    );
    let result = match runtime.eval(&form) {
        Ok(value) => value,
        Err(error) => panic!("missing LOAD did not signal a handled condition: {error}"),
    };
    assert!(
        runtime.format_result(result) == "T",
        "missing LOAD did not signal FILE-ERROR, result: {}",
        runtime.format_result(result)
    );
}

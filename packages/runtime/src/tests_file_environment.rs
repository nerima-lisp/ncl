use crate::Runtime;
use std::{
    fs,
    path::{Path, PathBuf},
};

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
fn compile_file_expands_with_open_file_around_do() {
    let input =
        std::env::temp_dir().join(format!("ncl-with-open-file-do-{}.lisp", std::process::id()));
    let source = "(defun repro-with-open-file-do (expected-failures if-does-not-exist) \
                  (with-open-file (stream expected-failures \
                                  :if-does-not-exist if-does-not-exist) \
                    (do ((name nil nil)) ((eq name nil)) name)))";
    if let Err(error) = fs::write(&input, source) {
        panic!("WITH-OPEN-FILE/DO regression source creation failed: {error}");
    }
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let result = runtime.compile_file(&input);
    let source_cleanup = fs::remove_file(&input);
    let output_cleanup = fs::remove_file(input.with_extension("fasl"));
    assert!(
        result.is_ok(),
        "COMPILE-FILE failed for WITH-OPEN-FILE/DO source: {result:?}"
    );
    if let Err(error) = source_cleanup {
        panic!("WITH-OPEN-FILE/DO regression source cleanup failed: {error}");
    }
    if let Err(error) = output_cleanup {
        panic!("WITH-OPEN-FILE/DO regression output cleanup failed: {error}");
    }
}

#[test]
fn compile_file_compiles_handler_case_with_multiple_clauses() {
    let input = std::env::temp_dir().join(format!("ncl-handler-case-{}.lisp", std::process::id()));
    let source =
        "(defun repro-handler-case (input) (handler-case input (warning () nil) (error () nil)))";
    if let Err(error) = fs::write(&input, source) {
        panic!("HANDLER-CASE regression source creation failed: {error}");
    }
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let result = runtime.compile_file(&input);
    let source_cleanup = fs::remove_file(&input);
    let output_cleanup = fs::remove_file(input.with_extension("fasl"));
    assert!(
        result.is_ok(),
        "COMPILE-FILE failed for a minimal HANDLER-CASE function: {result:?}"
    );
    if let Err(error) = source_cleanup {
        panic!("HANDLER-CASE regression source cleanup failed: {error}");
    }
    if let Err(error) = output_cleanup {
        panic!("HANDLER-CASE regression output cleanup failed: {error}");
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
fn nested_fasl_load_restores_evaluator_for_following_empty_source_load() {
    let directory =
        std::env::temp_dir().join(format!("ncl-nested-fasl-load-{}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&directory) {
        panic!("nested LOAD regression directory creation failed: {error}");
    }
    let source = directory.join("compiled.lisp");
    let fasl = source.with_extension("fasl");
    let empty = directory.join("empty.lisp");
    let result = (|| {
        fs::write(&source, "(setq *nested-fasl-load-observed* t)")
            .map_err(|error| format!("COMPILE-FILE source creation failed: {error}"))?;
        fs::write(&empty, "")
            .map_err(|error| format!("empty LOAD source creation failed: {error}"))?;
        let mut runtime =
            Runtime::new().map_err(|error| format!("runtime initialization failed: {error}"))?;
        runtime
            .eval("(defparameter *nested-fasl-load-observed* nil)")
            .map_err(|error| format!("LOAD regression setup failed: {error}"))?;
        runtime
            .compile_file(&source)
            .map_err(|error| format!("COMPILE-FILE failed: {error}"))?;
        let forms = format!(
            "(load {}) (load {})",
            lisp_string(&fasl),
            lisp_string(&empty)
        );
        let loaded = runtime
            .eval(&forms)
            .map_err(|error| format!("FASL and empty-source LOAD sequence failed: {error}"))?;
        let observed = runtime
            .eval("(eq *nested-fasl-load-observed* t)")
            .map_err(|error| format!("FASL result check failed: {error}"))?;
        Ok::<_, String>((
            runtime.format_result(loaded),
            runtime.format_result(observed),
        ))
    })();
    if let Err(error) = fs::remove_dir_all(&directory) {
        panic!("nested LOAD regression cleanup failed: {error}");
    }
    let (loaded, observed) = result.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(loaded, "T");
    assert_eq!(observed, "T");
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
    assert_eq!(runtime.format_result(result), "T");
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
fn load_accepts_pathname_designators() {
    let filename = format!("ncl-load-pathname-designator-{}.lisp", std::process::id());
    let input = std::env::temp_dir().join(filename);
    if let Err(error) = fs::write(&input, ":pathname-load-ok") {
        panic!("LOAD source creation failed: {error}");
    }
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let form = format!("(load (pathname {}))", lisp_string(&input));
    let result = match runtime.eval(&form) {
        Ok(value) => value,
        Err(error) => panic!("LOAD pathname designator failed: {error}"),
    };
    assert_eq!(runtime.format_result(result), "T");
    if let Err(error) = fs::remove_file(&input) {
        panic!("LOAD source cleanup failed: {error}");
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

fn test_value<T, E: std::fmt::Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

#[test]
fn r5_load_in_package_changes_later_reads_and_restores_the_caller_package() -> Result<(), String> {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-in-package-{}.lisp",
        std::process::id()
    ));
    let contents = "(|IN-PACKAGE| \"R5-LOAD-IN-PACKAGE\") \
                    (|SETQ| |*READTABLE*| |*R5-LOAD-IN-PACKAGE-UPCASE-READTABLE*|) \
                    'MiXeD";
    test_value(fs::write(&source, contents))?;
    let mut runtime = test_value(Runtime::new())?;
    test_value(runtime.eval(
        "(make-package \"R5-LOAD-IN-PACKAGE\" \
                :use (list (find-package \"COMMON-LISP\"))) \
             (defparameter *r5-load-in-package-caller-package* *package*) \
             (defparameter *r5-load-in-package-upcase-readtable* *readtable*) \
             (defparameter *r5-load-in-package-downcase-readtable* (copy-readtable))",
    ))?;
    test_value(runtime.eval(
        "(import (list (find-symbol \"*R5-LOAD-IN-PACKAGE-UPCASE-READTABLE*\" \
                                \"COMMON-LISP-USER\")) \
                    (find-package \"R5-LOAD-IN-PACKAGE\"))",
    ))?;
    let downcase = test_value(runtime.eval("*r5-load-in-package-downcase-readtable*"))?;
    test_value(runtime.context.write_object_slot(
        downcase,
        ncl_object::readtable_offset::CASE,
        ncl_object::Word::fixnum(1),
    ))?;
    test_value(runtime.eval("(setq *readtable* *r5-load-in-package-downcase-readtable*)"))?;

    let loaded = test_value(runtime.eval(&format!("(load {})", lisp_string(&source))))?;
    assert_eq!(runtime.format_result(loaded), "T");
    let observed = test_value(runtime.eval(
        "(let ((symbol (find-symbol \"MIXED\" \"R5-LOAD-IN-PACKAGE\"))) \
               (and symbol \
                    (string= (symbol-name symbol) \"MIXED\") \
                    (eq (symbol-package symbol) (find-package \"R5-LOAD-IN-PACKAGE\")) \
                    (eq *package* *r5-load-in-package-caller-package*) \
                    (eq *readtable* *r5-load-in-package-downcase-readtable*)))",
    ))?;
    assert_eq!(runtime.format_result(observed), "T");
    test_value(fs::remove_file(&source))?;
    Ok(())
}

#[test]
fn r5_load_uses_and_restores_the_dynamic_readtable() -> Result<(), String> {
    let source =
        std::env::temp_dir().join(format!("ncl-r5-load-readtable-{}.lisp", std::process::id()));
    let contents = "(|SETQ| |*R5-LOAD-READTABLE-BEFORE*| 'MiXeD) \
                    (|SETQ| |*READTABLE*| |*R5-LOAD-UPCASE-READTABLE*|) \
                    (|SETQ| |*R5-LOAD-READTABLE-AFTER*| 'MiXeD)";
    test_value(fs::write(&source, contents))?;
    let mut runtime = test_value(Runtime::new())?;
    test_value(runtime.eval(
        "(defparameter *r5-load-upcase-readtable* *readtable*) \
             (defparameter *r5-load-downcase-readtable* (copy-readtable)) \
             (defparameter *r5-load-readtable-before* nil) \
             (defparameter *r5-load-readtable-after* nil)",
    ))?;
    let downcase = test_value(runtime.eval("*r5-load-downcase-readtable*"))?;
    test_value(runtime.context.write_object_slot(
        downcase,
        ncl_object::readtable_offset::CASE,
        ncl_object::Word::fixnum(1),
    ))?;
    test_value(runtime.eval("(setq *readtable* *r5-load-downcase-readtable*)"))?;

    test_value(runtime.load_file(&source))?;
    let before = test_value(runtime.eval("(symbol-name *r5-load-readtable-before*)"))?;
    let after = test_value(runtime.eval("(symbol-name *r5-load-readtable-after*)"))?;
    let restored = test_value(runtime.eval("(eq *readtable* *r5-load-downcase-readtable*)"))?;
    assert_eq!(runtime.format_result(before), "\"mixed\"");
    assert_eq!(runtime.format_result(after), "\"MIXED\"");
    assert_eq!(runtime.format_result(restored), "T");
    let output = test_value(runtime.eval(&format!(
        "(progv (list (intern \"*LOAD-PRINT*\" (find-package \"COMMON-LISP\")) \
                          (intern \"*LOAD-VERBOSE*\" (find-package \"COMMON-LISP\"))) \
                    '(t t) \
               (with-output-to-string (stream) \
                 (let ((*standard-output* stream)) (load {}))))",
        lisp_string(&source)
    )))?;
    let output = runtime.format_result(output);
    assert!(output.contains("MIXED"));
    assert!(output.contains("ncl-r5-load-readtable"));
    let relative_fixture =
        PathBuf::from(format!("ncl-r5-relative-load-{}.lisp", std::process::id()));
    let current_dir = test_value(std::env::current_dir())?;
    test_value(fs::write(current_dir.join(&relative_fixture), "42"))?;
    test_value(runtime.load_file(&relative_fixture))?;
    test_value(fs::remove_file(&relative_fixture))?;
    test_value(fs::remove_file(&source))?;
    Ok(())
}

#[test]
fn r5_compile_file_uses_and_restores_the_dynamic_readtable() -> Result<(), String> {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-compile-file-readtable-{}.lisp",
        std::process::id()
    ));
    let contents = "(|EVAL-WHEN| (:|COMPILE-TOPLEVEL|) (|SETQ| |*R5-COMPILE-READTABLE-BEFORE*| 'MiXeD)) \
                    (|EVAL-WHEN| (:|COMPILE-TOPLEVEL|) (|SETQ| |*READTABLE*| |*R5-COMPILE-UPCASE-READTABLE*|)) \
                    (|EVAL-WHEN| (:|COMPILE-TOPLEVEL|) (|SETQ| |*R5-COMPILE-READTABLE-AFTER*| 'MiXeD))";
    test_value(fs::write(&source, contents))?;
    let mut runtime = test_value(Runtime::new())?;
    test_value(runtime.eval(
        "(defparameter *r5-compile-upcase-readtable* *readtable*) \
             (defparameter *r5-compile-downcase-readtable* (copy-readtable)) \
             (defparameter *r5-compile-readtable-before* nil) \
             (defparameter *r5-compile-readtable-after* nil)",
    ))?;
    let downcase = test_value(runtime.eval("*r5-compile-downcase-readtable*"))?;
    test_value(runtime.context.write_object_slot(
        downcase,
        ncl_object::readtable_offset::CASE,
        ncl_object::Word::fixnum(1),
    ))?;
    test_value(runtime.eval("(setq *readtable* *r5-compile-downcase-readtable*)"))?;

    test_value(runtime.compile_file(&source))?;
    let before = test_value(runtime.eval("(symbol-name *r5-compile-readtable-before*)"))?;
    let after = test_value(runtime.eval("(symbol-name *r5-compile-readtable-after*)"))?;
    let restored = test_value(runtime.eval("(eq *readtable* *r5-compile-downcase-readtable*)"))?;
    assert_eq!(runtime.format_result(before), "\"mixed\"");
    assert_eq!(runtime.format_result(after), "\"MIXED\"");
    assert_eq!(runtime.format_result(restored), "T");
    test_value(fs::remove_file(&source))?;
    test_value(fs::remove_file(source.with_extension("fasl")))?;
    Ok(())
}

#[test]
fn r5_load_fasl_reads_in_the_dynamic_package_and_restores_the_caller_package() -> Result<(), String>
{
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-fasl-package-{}.lisp",
        std::process::id()
    ));
    let contents = "(|SETQ| |*R5-FASL-INITIAL-READTABLE-RESULT*| 'MiXeD \
                    |*READTABLE*| |*R5-FASL-UPCASE-READTABLE*|) \
                    'MiXeD 'R5-FASL-PACKAGE-MARKER";
    test_value(fs::write(&source, contents))?;
    let mut runtime = test_value(Runtime::new())?;
    test_value(runtime.eval(
        "(make-package \"R5-FASL-LOAD-TARGET\" \
                :use (list (find-package \"COMMON-LISP\"))) \
             (defparameter *r5-fasl-caller-package* *package*) \
             (defparameter *r5-fasl-caller-readtable* *readtable*) \
             (defparameter *r5-fasl-upcase-readtable* *readtable*) \
             (defparameter *r5-fasl-downcase-readtable* (copy-readtable)) \
             (setq *package* (find-package \"R5-FASL-LOAD-TARGET\"))",
    ))?;
    test_value(runtime.eval("(defparameter |*R5-FASL-INITIAL-READTABLE-RESULT*| nil)"))?;
    test_value(runtime.eval("(setq *package* (find-package \"COMMON-LISP-USER\"))"))?;
    test_value(runtime.eval(
        "(import (list (find-symbol \"*R5-FASL-UPCASE-READTABLE*\" \
                                \"COMMON-LISP-USER\")) \
                    (find-package \"R5-FASL-LOAD-TARGET\"))",
    ))?;
    let downcase = test_value(runtime.eval("*r5-fasl-downcase-readtable*"))?;
    test_value(runtime.context.write_object_slot(
        downcase,
        ncl_object::readtable_offset::CASE,
        ncl_object::Word::fixnum(1),
    ))?;
    test_value(runtime.compile_file(&source))?;
    let fasl = source.with_extension("fasl");
    let form = format!(
        "(let ((*package* (find-package \"R5-FASL-LOAD-TARGET\")) \
                   (*readtable* *r5-fasl-downcase-readtable*)) \
           (load {} :external-format :r5-unsupported-format) \
           (and (eq *package* (find-package \"R5-FASL-LOAD-TARGET\")) \
                (eq *readtable* *r5-fasl-downcase-readtable*)))",
        lisp_string(&fasl)
    );
    let load_bindings_restored = test_value(runtime.eval(&form))?;
    assert_eq!(runtime.format_result(load_bindings_restored), "T");
    let observed = test_value(runtime.eval(
        "(and (find-symbol \"mixed\" \"R5-FASL-LOAD-TARGET\") \
                  (find-symbol \"MIXED\" \"R5-FASL-LOAD-TARGET\") \
                  (find-symbol \"R5-FASL-PACKAGE-MARKER\" \"R5-FASL-LOAD-TARGET\") \
                  (string= (symbol-name \
                      (symbol-value (find-symbol \
                          \"*R5-FASL-INITIAL-READTABLE-RESULT*\" \
                          \"R5-FASL-LOAD-TARGET\"))) \"mixed\") \
                  (eq *package* *r5-fasl-caller-package*) \
                  (eq *readtable* *r5-fasl-caller-readtable*))",
    ))?;
    assert_eq!(runtime.format_result(observed), "T");
    test_value(fs::remove_file(&source))?;
    test_value(fs::remove_file(fasl))?;
    Ok(())
}

#[test]
fn r5_load_restores_package_and_readtable_after_unwind() -> Result<(), String> {
    let source =
        std::env::temp_dir().join(format!("ncl-r5-load-unwind-{}.lisp", std::process::id()));
    let contents = "(setq *readtable* (copy-readtable)) \
                    (setq *package* (find-package \"R5-LOAD-UNWIND-INNER\")) \
                    (throw 'r5-load-unmatched-tag :payload)";
    test_value(fs::write(&source, contents))?;
    let mut runtime = test_value(Runtime::new())?;
    test_value(runtime.eval(
        "(make-package \"R5-LOAD-UNWIND-OUTER\" :use (list (find-package \"COMMON-LISP\"))) \
             (make-package \"R5-LOAD-UNWIND-INNER\" :use (list (find-package \"COMMON-LISP\"))) \
             (defparameter *r5-load-unwind-package* (find-package \"R5-LOAD-UNWIND-OUTER\")) \
             (defparameter *r5-load-unwind-readtable* *readtable*) \
             (setq *package* *r5-load-unwind-package*)",
    ))?;

    assert!(runtime.load_file(&source).is_err());
    let _ = runtime.context.take_non_local_exit();
    let restored = test_value(runtime.eval(
        "(and (eq *package* *r5-load-unwind-package*) \
                  (eq *readtable* *r5-load-unwind-readtable*))",
    ))?;
    assert_eq!(runtime.format_result(restored), "T");
    assert!(
        runtime
            .eval(&format!(
                "(progv (list '*package*) '(nil) (load {}))",
                lisp_string(&source)
            ))
            .is_err()
    );
    let _ = runtime.context.take_non_local_exit();
    assert!(
        runtime
            .eval(&format!(
                "(progv (list '*readtable*) '(nil) (load {}))",
                lisp_string(&source)
            ))
            .is_err()
    );
    let _ = runtime.context.take_non_local_exit();

    let common_lisp = runtime
        .object
        .find_package(&runtime.context, "COMMON-LISP")
        .ok_or_else(|| "COMMON-LISP package is missing".to_owned())?;
    let common_lisp = ncl_object::Package::from_word(common_lisp);
    test_value(common_lisp.set_locked(&mut runtime.context, true))?;
    assert!(crate::file_environment::register(&mut runtime.context, &runtime.object).is_err());
    assert!(runtime.load_file(&source).is_err());
    test_value(common_lisp.set_locked(&mut runtime.context, false))?;
    test_value(fs::remove_file(&source))?;
    Ok(())
}

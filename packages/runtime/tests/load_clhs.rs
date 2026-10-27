#![allow(missing_docs)]

use ncl_runtime::Runtime;
use std::{fs, path::Path};

fn lisp_string(value: &Path) -> String {
    format!("{:?}", value.to_string_lossy())
}

#[test]
fn r5_load_reads_in_the_dynamic_package_and_restores_it() {
    let source =
        std::env::temp_dir().join(format!("ncl-r5-load-package-{}.lisp", std::process::id()));
    let contents = "(setq *r5-load-outer-marker* 'outer-marker) \
                    (setq *package* (find-package \"R5-LOAD-INNER\")) \
                    (setq *r5-load-inner-marker* 'inner-marker)";
    if let Err(error) = fs::write(&source, contents) {
        panic!("LOAD package regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .eval(
            "(make-package \"R5-LOAD-OUTER\" :use (list (find-package \"COMMON-LISP\"))) \
             (make-package \"R5-LOAD-INNER\" :use (list (find-package \"COMMON-LISP\"))) \
             (setq *package* (find-package \"R5-LOAD-OUTER\"))",
        )
        .unwrap_or_else(|error| panic!("LOAD package setup failed: {error}"));

    runtime
        .load_file(&source)
        .unwrap_or_else(|error| panic!("LOAD package regression failed: {error}"));
    let observed = runtime
        .eval(
            "(and (find-symbol \"*R5-LOAD-OUTER-MARKER*\" \"R5-LOAD-OUTER\") \
                  (find-symbol \"*R5-LOAD-INNER-MARKER*\" \"R5-LOAD-INNER\") \
                  (eq *package* (find-package \"R5-LOAD-OUTER\")))",
        )
        .unwrap_or_else(|error| panic!("LOAD package observation failed: {error}"));
    assert_eq!(runtime.format_result(observed), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD package regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_compile_file_reads_in_the_dynamic_package_and_restores_it() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-compile-file-package-{}.lisp",
        std::process::id()
    ));
    let contents = "(eval-when (:compile-toplevel) (setq *r5-compile-outer-marker* 'outer-marker)) \
                    (eval-when (:compile-toplevel) (setq *package* (find-package \"R5-COMPILE-INNER\"))) \
                    (eval-when (:compile-toplevel) (setq *r5-compile-inner-marker* 'inner-marker))";
    if let Err(error) = fs::write(&source, contents) {
        panic!("COMPILE-FILE package regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .eval(
            "(make-package \"R5-COMPILE-OUTER\" :use (list (find-package \"COMMON-LISP\"))) \
             (make-package \"R5-COMPILE-INNER\" :use (list (find-package \"COMMON-LISP\"))) \
             (setq *package* (find-package \"R5-COMPILE-OUTER\"))",
        )
        .unwrap_or_else(|error| panic!("COMPILE-FILE package setup failed: {error}"));

    runtime
        .compile_file(&source)
        .unwrap_or_else(|error| panic!("COMPILE-FILE package regression failed: {error}"));
    let observed = runtime
        .eval(
            "(and (find-symbol \"*R5-COMPILE-OUTER-MARKER*\" \"R5-COMPILE-OUTER\") \
                  (find-symbol \"*R5-COMPILE-INNER-MARKER*\" \"R5-COMPILE-INNER\") \
                  (eq *package* (find-package \"R5-COMPILE-OUTER\")))",
        )
        .unwrap_or_else(|error| panic!("COMPILE-FILE package observation failed: {error}"));
    assert_eq!(runtime.format_result(observed), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("COMPILE-FILE package regression source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_file(source.with_extension("fasl")) {
        panic!("COMPILE-FILE package regression output cleanup failed: {error}");
    }
}

#[test]
fn r5_load_builtin_returns_true_after_a_source_load() {
    let source = std::env::temp_dir().join(format!("ncl-r5-load-true-{}.lisp", std::process::id()));
    if let Err(error) = fs::write(&source, "42") {
        panic!("LOAD return regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let form = format!("(load {})", lisp_string(&source));
    let result = runtime
        .eval(&form)
        .unwrap_or_else(|error| panic!("LOAD return regression failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD return regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_builtin_ignores_external_format_for_fasl_and_returns_true() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-fasl-format-{}.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&source, "42") {
        panic!("FASL LOAD regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .compile_file(&source)
        .unwrap_or_else(|error| panic!("FASL LOAD regression compilation failed: {error}"));
    let fasl = source.with_extension("fasl");
    let form = format!(
        "(load {} :external-format :r5-unsupported-format)",
        lisp_string(&fasl)
    );
    let result = runtime
        .eval(&form)
        .unwrap_or_else(|error| panic!("FASL LOAD external-format regression failed: {error}"));
    assert_eq!(runtime.format_result(result), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("FASL LOAD regression source cleanup failed: {error}");
    }
    if let Err(error) = fs::remove_file(fasl) {
        panic!("FASL LOAD regression output cleanup failed: {error}");
    }
}

#[test]
fn r5_load_print_option_writes_form_values_to_standard_output() {
    let source =
        std::env::temp_dir().join(format!("ncl-r5-load-print-{}.lisp", std::process::id()));
    if let Err(error) = fs::write(&source, "42") {
        panic!("LOAD :PRINT regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let form = format!(
        "(with-output-to-string (stream) \
           (let ((*standard-output* stream)) (load {} :print t)))",
        lisp_string(&source)
    );
    let output = runtime
        .eval(&form)
        .unwrap_or_else(|error| panic!("LOAD :PRINT output capture failed: {error}"));
    assert!(
        runtime.format_result(output).contains("42"),
        "LOAD :PRINT did not capture the loaded form value: {}",
        runtime.format_result(output)
    );
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD :PRINT regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_verbose_option_writes_a_message_to_standard_output() {
    let source =
        std::env::temp_dir().join(format!("ncl-r5-load-verbose-{}.lisp", std::process::id()));
    if let Err(error) = fs::write(&source, "42") {
        panic!("LOAD :VERBOSE regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let source_name = source
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_else(|| panic!("LOAD :VERBOSE source filename is not UTF-8"));
    let form = format!(
        "(let ((output (with-output-to-string (stream) \
                        (let ((*standard-output* stream)) (load {} :verbose t))))) \
           (and (= (char-code (char output 0)) 59) \
                (search {} output) \
                t))",
        lisp_string(&source),
        lisp_string(Path::new(source_name))
    );
    let output = runtime
        .eval(&form)
        .unwrap_or_else(|error| panic!("LOAD :VERBOSE output capture failed: {error}"));
    assert_eq!(runtime.format_result(output), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD :VERBOSE regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_verbose_option_defaults_from_load_verbose_and_explicit_nil_overrides() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-verbose-default-{}.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&source, "42") {
        panic!("LOAD-VERBOSE default regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let path = lisp_string(&source);
    let default_form = format!(
        "(progv (list (intern \"*LOAD-VERBOSE*\" (find-package \"COMMON-LISP\"))) '(t) \
           (with-output-to-string (stream) \
             (let ((*standard-output* stream)) (load {path}))))"
    );
    let default_output = runtime
        .eval(&default_form)
        .unwrap_or_else(|error| panic!("LOAD-VERBOSE default capture failed: {error}"));
    let default_output = runtime.format_result(default_output);
    let source_name = source
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_else(|| panic!("LOAD-VERBOSE source filename is not UTF-8"));
    assert!(
        default_output.contains(source_name),
        "LOAD ignored true *LOAD-VERBOSE* when :VERBOSE was omitted: {default_output}"
    );

    let override_form = format!(
        "(progv (list (intern \"*LOAD-VERBOSE*\" (find-package \"COMMON-LISP\"))) '(t) \
           (with-output-to-string (stream) \
             (let ((*standard-output* stream)) (load {path} :verbose nil))))"
    );
    let override_output = runtime
        .eval(&override_form)
        .unwrap_or_else(|error| panic!("LOAD-VERBOSE explicit NIL capture failed: {error}"));
    assert_eq!(runtime.format_result(override_output), "\"\"");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD-VERBOSE default regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_print_option_defaults_from_load_print_and_explicit_nil_overrides() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-print-default-{}.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&source, "42") {
        panic!("LOAD-PRINT default regression source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let path = lisp_string(&source);
    let default_form = format!(
        "(progv (list (intern \"*LOAD-PRINT*\" (find-package \"COMMON-LISP\"))) '(t) \
           (with-output-to-string (stream) \
             (let ((*standard-output* stream)) (load {path}))))"
    );
    let default_output = runtime
        .eval(&default_form)
        .unwrap_or_else(|error| panic!("LOAD-PRINT default capture failed: {error}"));
    let default_output = runtime.format_result(default_output);
    assert!(
        default_output.contains("42"),
        "LOAD ignored true *LOAD-PRINT* when :PRINT was omitted: {default_output}"
    );

    let override_form = format!(
        "(progv (list (intern \"*LOAD-PRINT*\" (find-package \"COMMON-LISP\"))) '(t) \
           (with-output-to-string (stream) \
             (let ((*standard-output* stream)) (load {path} :print nil))))"
    );
    let override_output = runtime
        .eval(&override_form)
        .unwrap_or_else(|error| panic!("LOAD-PRINT explicit NIL capture failed: {error}"));
    assert_eq!(runtime.format_result(override_output), "\"\"");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD-PRINT default regression source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_uses_the_shadowed_handler_case_macro_in_the_dynamic_package() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-shadow-handler-case-{}.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&source, "(handler-case 42 (error () nil))") {
        panic!("LOAD shadowed HANDLER-CASE source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    runtime
        .eval(
            "(make-package \"R5-LOAD-SHADOW-HANDLER-CASE\" \
                :use (list (find-package \"COMMON-LISP\"))) \
             (shadow (list \"HANDLER-CASE\") \
                     (find-package \"R5-LOAD-SHADOW-HANDLER-CASE\")) \
             (setq *package* (find-package \"R5-LOAD-SHADOW-HANDLER-CASE\"))",
        )
        .unwrap_or_else(|error| panic!("LOAD shadowed HANDLER-CASE package setup failed: {error}"));
    runtime
        .eval(
            "(defparameter \
                R5-LOAD-SHADOW-HANDLER-CASE::*R5-LOAD-SHADOW-HANDLER-CASE-RESULT* nil) \
             (defmacro R5-LOAD-SHADOW-HANDLER-CASE::HANDLER-CASE (&rest clauses) \
               '(setq R5-LOAD-SHADOW-HANDLER-CASE::*R5-LOAD-SHADOW-HANDLER-CASE-RESULT* \
                      :shadow-handler-case))",
        )
        .unwrap_or_else(|error| panic!("LOAD shadowed HANDLER-CASE macro setup failed: {error}"));
    runtime
        .eval("(setq *package* (find-package \"COMMON-LISP-USER\"))")
        .unwrap_or_else(|error| panic!("LOAD caller package restoration failed: {error}"));

    let form = format!(
        "(let ((*package* (find-package \"R5-LOAD-SHADOW-HANDLER-CASE\"))) \
           (load {}))",
        lisp_string(&source)
    );
    runtime
        .eval(&form)
        .unwrap_or_else(|error| panic!("LOAD shadowed HANDLER-CASE regression failed: {error}"));
    let observed = runtime
        .eval(
            "(and (eq (symbol-value (find-symbol \
                      \"*R5-LOAD-SHADOW-HANDLER-CASE-RESULT*\" \
                      \"R5-LOAD-SHADOW-HANDLER-CASE\")) \
                    :shadow-handler-case) \
                  (eq *package* (find-package \"COMMON-LISP-USER\")))",
        )
        .unwrap_or_else(|error| panic!("LOAD shadowed HANDLER-CASE observation failed: {error}"));
    assert_eq!(runtime.format_result(observed), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD shadowed HANDLER-CASE source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_preserves_macrolet_and_symbol_macrolet_for_each_top_level_form() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-top-level-macrolet-{}.lisp",
        std::process::id()
    ));
    let contents = "(progn) \
                    (defparameter *r5-load-top-level-locally-value* nil) \
                    (locally (declare (special *r5-load-top-level-locally-value*)) \
                      (setq *r5-load-top-level-locally-value* :locally)) \
                    (defparameter *r5-load-top-level-lambda-value* nil) \
                    ((lambda () (setq *r5-load-top-level-lambda-value* :lambda))) \
                    (defparameter *r5-load-top-level-macro-value* nil) \
                    (macrolet ((r5-load-remember (value) \
                                (list 'setq '*r5-load-top-level-macro-value* value))) \
                      (r5-load-remember :first) \
                      (r5-load-remember :second)) \
                    (defparameter *r5-load-top-level-symbol-value* 10) \
                    (defparameter *r5-load-top-level-symbol-first* nil) \
                    (defparameter *r5-load-top-level-symbol-second* nil) \
                    (symbol-macrolet ((r5-load-top-level-symbol-alias \
                                       *r5-load-top-level-symbol-value*)) \
                      (setq *r5-load-top-level-symbol-first* \
                            (+ r5-load-top-level-symbol-alias 1)) \
                      (setq *r5-load-top-level-symbol-second* \
                            (+ r5-load-top-level-symbol-alias 2))) \
                    (locally 42)";
    if let Err(error) = fs::write(&source, contents) {
        panic!("LOAD top-level macrolet source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));

    let loaded = runtime
        .load_file(&source)
        .unwrap_or_else(|error| panic!("LOAD top-level macrolet forms failed: {error}"));
    assert_eq!(runtime.format_result(loaded), "42");
    let observed = runtime
        .eval(
            "(and (eq *r5-load-top-level-locally-value* :locally) \
                  (eq *r5-load-top-level-lambda-value* :lambda) \
                  (eq *r5-load-top-level-macro-value* :second) \
                  (= *r5-load-top-level-symbol-first* 11) \
                  (= *r5-load-top-level-symbol-second* 12))",
        )
        .unwrap_or_else(|error| panic!("LOAD top-level macrolet results failed: {error}"));
    assert_eq!(runtime.format_result(observed), "T");
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD top-level macrolet source cleanup failed: {error}");
    }
}

#[test]
fn r5_load_rejects_macrolet_without_a_definition_list() {
    let source = std::env::temp_dir().join(format!(
        "ncl-r5-load-macrolet-without-definitions-{}.lisp",
        std::process::id()
    ));
    if let Err(error) = fs::write(&source, "(macrolet)") {
        panic!("LOAD malformed MACROLET source creation failed: {error}");
    }
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));

    let result = runtime.load_file(&source);
    assert!(
        result.is_err(),
        "LOAD accepted MACROLET without a definition list"
    );
    if let Err(error) = fs::remove_file(&source) {
        panic!("LOAD malformed MACROLET source cleanup failed: {error}");
    }
}

fn assert_load_condition_probe(
    runtime: &mut Runtime,
    route: &str,
    compile: bool,
    form: &str,
    failure: &str,
) {
    let result = if compile {
        runtime.compile(form)
    } else {
        runtime.eval(form)
    };
    let value = result.unwrap_or_else(|error| {
        panic!("LOAD {route} condition probe failed unexpectedly: {error}")
    });
    assert_eq!(runtime.format_result(value), "T", "LOAD {route} {failure}");
}

#[test]
fn load_builtin_classifies_errors_in_eval_and_compile_paths() {
    let stem = format!("ncl-load-error-class-{}", std::process::id());
    let missing = std::env::temp_dir().join(format!("{stem}-missing.lisp"));
    let reader_error = std::env::temp_dir().join(format!("{stem}-reader.lisp"));
    let lowering_error = std::env::temp_dir().join(format!("{stem}-lowering.lisp"));
    let unsupported_format = std::env::temp_dir().join(format!("{stem}-format.lisp"));
    let simple_error = std::env::temp_dir().join(format!("{stem}-simple.lisp"));
    let type_error = std::env::temp_dir().join(format!("{stem}-type.lisp"));
    if missing.exists() {
        fs::remove_file(&missing)
            .unwrap_or_else(|error| panic!("stale missing LOAD fixture cleanup failed: {error}"));
    }
    fs::write(&reader_error, ")")
        .unwrap_or_else(|error| panic!("LOAD reader fixture creation failed: {error}"));
    fs::write(&lowering_error, "(go ncl-load-error-class-missing-tag)")
        .unwrap_or_else(|error| panic!("LOAD lowering fixture creation failed: {error}"));
    fs::write(&unsupported_format, "(values)")
        .unwrap_or_else(|error| panic!("LOAD format fixture creation failed: {error}"));
    fs::write(&simple_error, "(error \"boom\")")
        .unwrap_or_else(|error| panic!("LOAD simple-error fixture creation failed: {error}"));
    fs::write(&type_error, "(car 1)")
        .unwrap_or_else(|error| panic!("LOAD type-error fixture creation failed: {error}"));

    let missing_form = format!(
        concat!(
            "(handler-case (progn (load {path}) :no-error) ",
            "(error (condition) ",
            "(and (typep condition 'file-error) ",
            "(stringp (ncl-ext::condition-slot-ref condition 0)) ",
            "(string= (ncl-ext::condition-slot-ref condition 0) {path}))))",
        ),
        path = lisp_string(&missing),
    );
    let reader_form = format!(
        concat!(
            "(handler-case (progn (load {}) :no-error) ",
            "(error (condition) ",
            "(and (typep condition 'reader-error) ",
            "(null (ncl-ext::condition-slot-ref condition 0)) ",
            "(string= (ncl-ext::condition-slot-ref condition 1) ",
            "\"unmatched right parenthesis\"))))",
        ),
        lisp_string(&reader_error),
    );
    let lowering_form = format!(
        concat!(
            "(handler-case (progn (load {}) :no-error) ",
            "(simple-error () t))",
        ),
        lisp_string(&lowering_error),
    );
    let unsupported_format_form = format!(
        concat!(
            "(handler-case (progn (load {} :external-format :utf-8) :no-error) ",
            "(simple-error () t))",
        ),
        lisp_string(&unsupported_format),
    );
    let simple_error_form = format!(
        "(handler-case (progn (load {}) :no-error) (simple-error () t))",
        lisp_string(&simple_error),
    );
    let type_error_form = format!(
        "(handler-case (progn (load {}) :no-error) (type-error () t))",
        lisp_string(&type_error),
    );

    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    for (route, compile) in [("eval", false), ("compile", true)] {
        for (form, failure) in [
            (
                &missing_form,
                "FILE-ERROR did not retain its pathname in slot 0",
            ),
            (&reader_form, "did not preserve READER-ERROR detail"),
            (
                &lowering_form,
                "lowering failure was not preserved as SIMPLE-ERROR",
            ),
            (
                &unsupported_format_form,
                "unsupported external format was not preserved as SIMPLE-ERROR",
            ),
            (&simple_error_form, "source SIMPLE-ERROR was not preserved"),
            (&type_error_form, "source TYPE-ERROR was not preserved"),
        ] {
            assert_load_condition_probe(&mut runtime, route, compile, form, failure);
        }
    }

    for path in [
        &reader_error,
        &lowering_error,
        &unsupported_format,
        &simple_error,
        &type_error,
    ] {
        fs::remove_file(path)
            .unwrap_or_else(|error| panic!("LOAD condition fixture cleanup failed: {error}"));
    }
}

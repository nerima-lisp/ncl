#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn eval_builtin_evaluates_a_form() {
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let value = match runtime.eval("(eval '(+ 20 22))") {
        Ok(value) => value,
        Err(error) => panic!("EVAL failed: {error}"),
    };
    assert_eq!(runtime.format_result(value), "42");
}

fn new_runtime() -> Runtime {
    match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error:?}"),
    }
}

fn eval_text(runtime: &mut Runtime, source: &str) -> String {
    match runtime.eval(source) {
        Ok(value) => runtime.format_result(value),
        Err(error) => panic!("evaluation failed: {error:?}"),
    }
}

fn eval_error(runtime: &mut Runtime, source: &str) -> ncl_runtime::RuntimeError {
    match runtime.eval(source) {
        Ok(_) => panic!("expected evaluation to fail"),
        Err(error) => error,
    }
}

#[test]
fn read_eval_evaluates_a_form_through_the_runtime_evaluator() {
    let mut runtime = new_runtime();
    let value = eval_text(&mut runtime, "#.(+ 1 2)");
    assert_eq!(value, "3");
}

#[test]
fn read_eval_nil_disables_sharp_dot_for_the_next_source_form() {
    let mut runtime = new_runtime();
    let error = eval_error(&mut runtime, "(setf *read-eval* nil) #.(+ 1 2)");
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Read(ncl_reader::ReadError::ReadEvalDisabled)
    ));
}

#[test]
fn read_eval_observes_changes_between_sharp_dot_dispatches() {
    let mut runtime = new_runtime();
    let error = eval_error(
        &mut runtime,
        "(list #.(progn (setf *read-eval* nil) 1) #.(+ 2 3))",
    );
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Read(ncl_reader::ReadError::ReadEvalDisabled)
    ));
    assert_eq!(eval_text(&mut runtime, "*READ-EVAL*"), "NIL");
}

#[test]
fn load_signals_reader_error_when_read_eval_is_nil() {
    let mut runtime = new_runtime();
    let path = std::env::temp_dir().join(format!(
        "ncl-read-eval-reader-error-{}.lisp",
        std::process::id()
    ));
    std::fs::write(&path, "#.(+ 1 2)")
        .unwrap_or_else(|error| panic!("write reader-error fixture: {error}"));
    let path_text = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let source = format!(
        "(let ((*read-eval* nil)) (handler-case (load \"{path_text}\") (reader-error () 17)))"
    );

    let result = runtime.eval(&source);
    std::fs::remove_file(&path)
        .unwrap_or_else(|error| panic!("remove reader-error fixture: {error}"));
    let value = result.unwrap_or_else(|error| panic!("reader-error handler failed: {error:?}"));
    assert_eq!(runtime.format_result(value), "17");
}

#[test]
fn read_suppress_consumes_sharp_dot_without_evaluation() {
    let mut runtime = new_runtime();
    let value = eval_text(
        &mut runtime,
        "(setf *read-suppress* t) #.(error \"suppressed form was evaluated\")",
    );
    assert_eq!(value, "NIL");
}

#[test]
fn read_eval_result_survives_collections_during_evaluation() {
    let mut runtime = new_runtime();
    runtime.set_gc_stress(true);
    let value = eval_text(
        &mut runtime,
        "'#.(progn (dotimes (i 32) (cons i nil)) '(+ 1 2))",
    );
    assert_eq!(value, "(+ 1 2)");
}

#[test]
fn read_base_rebinding_applies_to_the_next_source_form() {
    let mut runtime = new_runtime();
    let value = eval_text(&mut runtime, "(setf *read-base* 16) 10");
    assert_eq!(value, "16");
}

#[test]
fn read_eval_nested_in_dotted_list_survives_gc_stress() {
    let mut runtime = new_runtime();
    runtime.set_gc_stress(true);
    let value = eval_text(
        &mut runtime,
        "'((#.(progn (dotimes (i 32) (cons i nil)) '(+ 1 2)) . T))",
    );
    assert_eq!(value, "(((+ 1 2) . T))");
}

#[test]
fn read_eval_defaults_to_true_and_with_standard_io_syntax_binds_true() {
    let mut runtime = new_runtime();
    let default = eval_text(&mut runtime, "*READ-EVAL*");
    assert_eq!(default, "T");
    let default_float_format = eval_text(&mut runtime, "*READ-DEFAULT-FLOAT-FORMAT*");
    assert_eq!(default_float_format, "SINGLE-FLOAT");

    let value = eval_text(
        &mut runtime,
        "(progn (setf *read-eval* nil) (with-standard-io-syntax *read-eval*))",
    );
    assert_eq!(value, "T");

    let restored = eval_text(&mut runtime, "*READ-EVAL*");
    assert_eq!(restored, "NIL");
}

#[test]
fn with_standard_io_syntax_binds_standard_values_and_restores_outer_values() {
    let mut runtime = new_runtime();
    assert_eq!(eval_text(&mut runtime, "*PRINT-CASE*"), ":UPCASE");
    let source = r#"
        (let ((variables '(*package*
                                           *print-array* *print-base* *print-case* *print-circle*
                                           *print-escape* *print-gensym* *print-length* *print-level*
                                           *print-lines* *print-miser-width* *print-pprint-dispatch*
                                           *print-pretty* *print-radix* *print-readably* *print-right-margin*
                                           *read-base* *read-default-float-format* *read-eval*
                                           *read-suppress* *readtable*
                                           ncl-ext::*print-circle-not-shared*
                                           ncl-ext::*print-vector-length*)))
                          (progv variables
                              '(:outer-package nil 8 :downcase t nil nil 3 2 1 2 :outer-pprint t t nil 10
                                8 double-float nil t :outer-readtable t 4)
                            (and
                              (with-standard-io-syntax
                                (and
                                  (eq (symbol-value '*package*) (find-package "CL-USER"))
                                  (equal (mapcar #'symbol-value
                                                 '(*print-array* *print-base* *print-case* *print-circle*
                                                   *print-escape* *print-gensym* *print-length* *print-level*
                                                   *print-lines* *print-miser-width* *print-pretty* *print-radix*
                                                   *print-readably* *print-right-margin* *read-base*
                                                   *read-default-float-format* *read-eval* *read-suppress*
                                                   ncl-ext::*print-circle-not-shared*
                                                   ncl-ext::*print-vector-length*))
                                         '(t 10 :upcase nil t t nil nil nil nil nil nil t nil 10 single-float
                                           t nil nil nil))
                                  (equal (symbol-value '*print-pprint-dispatch*)
                                         (copy-pprint-dispatch nil))
                                  (and (not (eq (symbol-value '*readtable*) :outer-readtable)) (progn (copy-readtable (symbol-value '*readtable*)) t))))
                              (equal (mapcar #'symbol-value variables)
                                     '(:outer-package nil 8 :downcase t nil nil 3 2 1 2 :outer-pprint t t nil 10
                                       8 double-float nil t :outer-readtable t 4)))))
    "#;
    assert_eq!(eval_text(&mut runtime, source), "T");
}

#[test]
fn eval_builtin_preserves_native_type_error_condition() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval("(handler-case (eval '(car 1)) (type-error () t))")
        .unwrap_or_else(|error| panic!("EVAL type-error probe failed: {error}"));
    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn progv_assert_preserves_lexical_value_after_handler_region() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(progv '(cl:*compile-file-truename* cl:*load-truename*) (list nil #p\"init.lsp\") (let ((x 1)) (assert t) x))",
        )
        .unwrap_or_else(|error| panic!("form evaluation failed: {error}"));
    assert_eq!(runtime.format_result(value), "1");
}

#[test]
fn eval_preserves_zero_one_and_multiple_values_through_the() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    for (source, expected) in [
        ("(multiple-value-list (eval '(values)))", "NIL"),
        ("(multiple-value-list (eval '(values 42)))", "(42)"),
        ("(multiple-value-list (eval '(values 1 2 3)))", "(1 2 3)"),
        (
            "(multiple-value-list (eval '(the t (values 1 2))))",
            "(1 2)",
        ),
    ] {
        let value = runtime
            .eval(source)
            .unwrap_or_else(|error| panic!("EVAL failed for {source}: {error}"));
        assert_eq!(runtime.format_result(value), expected, "{source}");
    }
}

#[test]
fn eval_multiple_value_prog1_preserves_all_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(multiple-value-prog1 (values 1 2) nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(1 2)");
}

#[test]
fn eval_multiple_value_prog1_preserves_single_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(multiple-value-prog1 7 nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7)");
}

#[test]
fn eval_unwind_protect_preserves_all_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(unwind-protect (values 1 2) nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(1 2)");
}

#[test]
fn eval_unwind_protect_preserves_single_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(unwind-protect 7 nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7)");
}

#[test]
fn eval_value_capture_preserves_zero_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    for source in [
        "(multiple-value-list (eval '(multiple-value-prog1 (values) nil)))",
        "(multiple-value-list (eval '(unwind-protect (values) nil)))",
    ] {
        let value = runtime
            .eval(source)
            .unwrap_or_else(|error| panic!("EVAL failed for {source}: {error}"));
        assert_eq!(runtime.format_result(value), "NIL", "{source}");
    }
}

#[test]
fn eval_multiple_value_call_captures_single_value_arguments() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (multiple-value-call #'values 7 8))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7 8)");
}

#[test]
fn eval_undefined_function_is_caught_by_ignore_errors() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(ignore-errors (eval '(ncl-eval-missing-function)))")
        .unwrap_or_else(|error| panic!("IGNORE-ERRORS did not catch EVAL's condition: {error}"));
    assert_eq!(value, Word::NIL);
}

#[test]
fn eval_undefined_function_is_caught_by_handler_case() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(handler-case (eval '(ncl-eval-missing-function)) (undefined-function () 17))")
        .unwrap_or_else(|error| panic!("HANDLER-CASE did not catch EVAL's condition: {error}"));
    assert_eq!(runtime.format_result(value), "17");
}

#[test]
fn direct_undefined_function_is_caught_by_handler_case() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(handler-case (ncl-direct-missing-function) (error () 17))")
        .unwrap_or_else(|error| {
            panic!("HANDLER-CASE did not catch direct undefined function: {error}")
        });
    assert_eq!(runtime.format_result(value), "17");
}

#[test]
fn handled_direct_undefined_function_does_not_leak_condition() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(handler-case\n             (progn\n               (handler-case (ncl-direct-missing-function) (error () 17))\n               (copy-readtable *readtable* 17))\n             (type-error () 23))",
        )
        .unwrap_or_else(|error| panic!("condition leaked from handled undefined function: {error}"));
    assert_eq!(runtime.format_result(value), "23");
}

#[test]
fn handler_bind_evaluates_a_function_form_once() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(let ((evaluations 0) \
                   (handled nil)) \
               (handler-bind ((warning (progn \
                                         (setq evaluations (+ evaluations 1)) \
                                         #'(lambda (condition) \
                                             (setq handled t))))) \
                 (signal 'warning)) \
               (list evaluations handled))",
        )
        .unwrap_or_else(|error| {
            panic!("HANDLER-BIND failed to evaluate its handler form: {error}")
        });
    assert_eq!(runtime.format_result(value), "(1 T)");
}

#[test]
fn handler_bind_accepts_an_empty_handler_list() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(handler-bind nil 42)")
        .unwrap_or_else(|error| panic!("HANDLER-BIND with no handlers failed: {error}"));
    assert_eq!(runtime.format_result(value), "42");
}

#[test]
fn defstruct_accessor_is_a_setf_place() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(defstruct repro (slot nil)) \
             (let ((value (make-repro))) \
               (list (setf (repro-slot value) 42) (repro-slot value)))",
        )
        .unwrap_or_else(|error| panic!("DEFSTRUCT accessor SETF failed: {error}"));
    assert_eq!(runtime.format_result(value), "(42 42)");
}

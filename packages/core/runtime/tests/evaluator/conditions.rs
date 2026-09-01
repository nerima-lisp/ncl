use super::*;

#[test]
fn preserves_unmatched_control_transfers_and_conditions() {
    for source in [
        "(catch 'outer (catch 'inner (throw 'other 8)))",
        "(handler-case (error \"boom\") (type-error () 9))",
        "(handler-bind ((type-error (lambda (condition) condition))) (error \"boom\"))",
    ] {
        let error = Runtime::new().eval_source(source).must_fail();
        assert!(
            matches!(
                error,
                ncl_runtime::RuntimeError::Throw { .. } | ncl_runtime::RuntimeError::Signaled(_)
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn rejects_malformed_condition_and_restart_forms() {
    for source in [
        "(handler-case 1)",
        "(handler-case 1 handler)",
        "(handler-case 1 (error))",
        "(handler-case 1 (error value 9))",
        "(handler-case 1 (error (first second) 9))",
        "(handler-case 1 (error (1) 9))",
        "(handler-bind)",
        "(handler-bind handlers 1)",
        "(handler-bind ((error)))",
        "(handler-bind ((error (lambda () 1)) extra))",
        "(restart-bind)",
        "(restart-bind bindings 1)",
        "(restart-bind ((continue)))",
        "(restart-bind ((continue (lambda () 1)) extra))",
        "(restart-case 1)",
        "(restart-case 1 continue)",
        "(restart-case 1 (continue))",
        "(with-simple-restart)",
        "(with-simple-restart continue 1)",
        "(with-simple-restart (continue) 1)",
        "(with-condition-restarts 1 nil 1)",
        "(with-condition-restarts (make-condition 'error) 1 1)",
        "(with-condition-restarts (make-condition 'error) (list 1) 1)",
    ] {
        let result = Runtime::new().eval_source(source);
        assert!(
            result.is_err(),
            "expected failure for {source}, got {result:?}"
        );
    }
}

#[test]
fn condition_message_returns_the_constructed_message() {
    let result = Runtime::new()
        .eval_source("(condition-message (make-condition 'simple-condition :format-control \"value: ~A\" :format-arguments (list 7)))")
        .unwrap_or_else(|error| panic!("condition-message failed: {error}"));
    assert_eq!(
        result
            .last()
            .unwrap_or_else(|| panic!("no result"))
            .to_string(),
        "\"value: 7\""
    );
}

#[test]
fn type_error_accessors_read_standard_condition_slots() {
    let result = Runtime::new()
        .eval_source(
            "(list (type-error-datum (make-condition 'type-error :datum 7 :expected-type 'integer))
                   (type-error-expected-type (make-condition 'type-error :datum 7 :expected-type 'integer)))",
        )
        .unwrap_or_else(|error| panic!("type-error accessors failed: {error}"));
    assert_eq!(result.last().unwrap().to_string(), "(7 INTEGER)");
}

#[test]
fn unbound_variable_name_reads_standard_condition_slot() {
    let result = Runtime::new()
        .eval_source("(unbound-variable-name (make-condition 'unbound-variable :name 'missing))")
        .unwrap_or_else(|error| panic!("unbound-variable-name failed: {error}"));
    assert_eq!(result.last().unwrap().to_string(), "MISSING");
}

#[test]
fn arithmetic_error_accessors_read_standard_condition_slots() {
    let result = Runtime::new()
        .eval_source(
            "(let ((condition (make-condition 'arithmetic-error :operation '+ :operands (list 1 2))))
               (list (arithmetic-error-operation condition)
                     (arithmetic-error-operands condition)))",
        )
        .unwrap_or_else(|error| panic!("arithmetic-error accessors failed: {error}"));
    assert_eq!(result.last().unwrap().to_string(), "(+ (1 2))");
}

#[test]
fn file_error_pathname_reads_standard_condition_slot() {
    let result = Runtime::new()
        .eval_source("(file-error-pathname (make-condition 'file-error :pathname \"missing.lisp\"))")
        .unwrap_or_else(|error| panic!("file-error-pathname failed: {error}"));
    assert_eq!(result.last().unwrap().to_string(), "\"missing.lisp\"");
}

#[test]
fn package_error_package_reads_standard_condition_slot() {
    let result = Runtime::new()
        .eval_source("(package-name (package-error-package (make-condition 'package-error :package (make-package \"package-error-accessor-eval\"))))")
        .unwrap_or_else(|error| panic!("package-error-package failed: {error}"));
    assert_eq!(result.last().unwrap().to_string(), "\"PACKAGE-ERROR-ACCESSOR-EVAL\"");
}

#[test]
fn stream_error_stream_reads_standard_condition_slot() {
    let result = Runtime::new()
        .eval_source("(stream-error-stream (make-condition 'stream-error :stream (make-string-input-stream \"data\")))")
        .unwrap_or_else(|error| panic!("stream-error-stream failed: {error}"));
    assert_eq!(result.last().unwrap().type_name(), "STREAM");
}

#[test]
fn rejects_malformed_character_operations_at_their_boundaries() {
    for source in support::MALFORMED_CHARACTER_FORMS {
        Runtime::eval_source(&Runtime::new(), source).must_fail();
    }
}

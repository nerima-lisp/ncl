#[test]
fn compiled_propagates_unhandled_conditions_from_handler_scopes() {
    let cases = [
        "(handler-case (error \"boom\") (type-error (condition) condition))",
        "(handler-bind ((type-error (lambda (condition) condition))) (error \"boom\"))",
    ];

    for source in cases {
        let error = Runtime::new().eval_compiled_source(source).must_fail();
        assert!(
            matches!(error, RuntimeError::Signaled(_)),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn compiled_propagates_unmatched_control_transfers_from_scopes() {
    for source in [
        "(catch 'tag (throw 'other 9))",
        "(with-simple-restart (abort \"abort\") (invoke-restart 'other 9))",
        "(restart-bind ((abort (lambda () 42))) (invoke-restart 'other))",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_malformed_condition_primitives_from_table_cases() {
    for source in [
        "(error)",
        "(signal)",
        "(warn)",
        "(cerror)",
        "(cerror \"continue\")",
        "(signal (make-condition 'simple-condition) \"extra\")",
        "(warn (make-condition 'simple-warning) \"extra\")",
        "(cerror \"continue\" (make-condition 'simple-error) \"extra\")",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_malformed_with_condition_restarts_arguments() {
    let cases = [
        "(with-condition-restarts 1 nil 1)",
        "(with-condition-restarts (make-condition 'error) 1 1)",
        "(with-condition-restarts (make-condition 'error) (list 1) 1)",
    ];

    for source in cases {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_rejects_malformed_character_operations_at_their_boundaries() {
    for source in support::MALFORMED_CHARACTER_FORMS {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

use super::*;

use ncl_runtime::{Runtime, RuntimeError};

#[path = "support/evaluation.rs"]
mod support;

use support::{assert_interpreted_and_compiled, evaluate_compiled, evaluate_interpreted};

#[test]
fn values_returns_the_primary_value_in_a_single_value_context() {
    assert_interpreted_and_compiled("(values 7)", "7");
    assert_interpreted_and_compiled("(values 7 8)", "7");
}

#[test]
fn multiple_value_list_and_values_list_round_trip_value_sequences() {
    assert_interpreted_and_compiled(
        "(list
           (multiple-value-list (values 1 2 3))
           (multiple-value-list (values))
           (multiple-value-list (values nil))
           (multiple-value-list 7)
           (multiple-value-call #'list (values-list '(4 5))))",
        "((1 2 3) NIL (NIL) (7) (4 5))",
    );
    assert_interpreted_and_compiled(
        "(multiple-value-bind (first second third)
           (values-list '(6 7))
           (list first second third))",
        "(6 7 NIL)",
    );
}

#[test]
fn multiple_value_bind_binds_all_values_and_fills_missing_bindings_with_nil() {
    assert_interpreted_and_compiled(
        "(multiple-value-bind (first second third) (values 1 2 3)
           (list first second third))",
        "(1 2 3)",
    );
    assert_interpreted_and_compiled(
        "(multiple-value-bind (first second third) (values 1 2)
           (list first second third))",
        "(1 2 NIL)",
    );
}

#[test]
fn multiple_value_call_flattens_values_from_each_producer() {
    assert_interpreted_and_compiled(
        "(multiple-value-call #'list (values 1 2) (values 3 4))",
        "(1 2 3 4)",
    );
}

#[test]
fn multiple_value_prog1_retains_values_and_evaluates_tail_forms_in_order() {
    assert_interpreted_and_compiled(
        "(let ((marker nil))
           (list
             (multiple-value-call #'list
               (multiple-value-prog1 (values 1 2) (setq marker :done)))
             marker))",
        "((1 2) :DONE)",
    );
}

#[test]
fn zero_values_are_distinct_from_one_nil() {
    assert_interpreted_and_compiled(
        "(list
           (multiple-value-call #'list (values))
           (multiple-value-call #'list (values nil)))",
        "(NIL (NIL))",
    );
    assert_interpreted_and_compiled(
        "(list
           (multiple-value-call #'list
             (multiple-value-prog1 (values) 1))
           (multiple-value-call #'list
             (multiple-value-prog1 (values nil) 1)))",
        "(NIL (NIL))",
    );
}

#[test]
fn ignore_errors_preserves_successful_multiple_values_and_catches_errors() {
    assert_interpreted_and_compiled(
        "(multiple-value-bind (value condition) (ignore-errors (values 1 2)) (list value condition))",
        "(1 2)",
    );
    assert_interpreted_and_compiled(
        "(multiple-value-bind (value condition) (ignore-errors (+ 1 \"x\")) (list value (type-of condition)))",
        "(NIL CONDITION)",
    );
}

#[test]
fn eval_preserves_multiple_values_and_argument_spans_in_both_execution_modes() {
    assert_interpreted_and_compiled(
        "(multiple-value-bind (first second)
           (ignore-errors (eval '(values 8 9)))
           (list first second))",
        "(8 9)",
    );

    let source = "(ignore-errors (eval (values 1 2)))";
    assert_eq!(evaluate_interpreted(source), evaluate_compiled(source));
}

#[test]
fn block_and_return_from_share_dynamic_control_semantics() {
    assert_interpreted_and_compiled("(block done 42)", "42");
    assert_interpreted_and_compiled("(block done (return-from done 1) 2)", "1");
    assert_interpreted_and_compiled("(block :done (return-from :done 1) 2)", "1");
    assert_interpreted_and_compiled(
        "(multiple-value-bind (first second)
           (block done (return-from done (values 1 2)))
           (list first second))",
        "(1 2)",
    );
    assert_interpreted_and_compiled(
        "(block done (funcall (lambda () (return-from done 7))))",
        "7",
    );
    assert_interpreted_and_compiled(
        "(let ((f nil))
           (block done
             (setf f (lambda () (return-from done 7)))
             (block done (funcall f) 3)
             4))",
        "7",
    );
    assert_interpreted_and_compiled("(block done (block done (return-from done 1) 2) 3)", "3");
    assert_interpreted_and_compiled("(block outer (block inner (return-from outer 1) 2) 3)", "1");
    assert_interpreted_and_compiled("(block done (ignore-errors (return-from done 9)) 4)", "9");
}

#[test]
fn unmatched_return_from_propagates_in_both_execution_modes() {
    for (compiled, result) in [
        (
            false,
            Runtime::new().eval_source("(block done (return-from other 1))"),
        ),
        (
            true,
            Runtime::new().eval_compiled_source("(block done (return-from other 1))"),
        ),
    ] {
        match result {
            Err(RuntimeError::ReturnFrom { block, .. }) => {
                assert_eq!(block, "OTHER", "compiled={compiled}");
            }
            other => panic!("expected unmatched RETURN-FROM error, got {other:?}"),
        }
    }
}

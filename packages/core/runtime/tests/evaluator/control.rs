use super::*;

#[test]
fn evaluates_dotimes_empty_result_and_final_binding() {
    assert_eq!(
        evaluate("(dotimes (index 0 index) (setq ran t))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(dotimes (index -2 index) (setq ran t))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(dotimes (index 3 index) (setq last index))").to_string(),
        "3"
    );
}

#[test]
fn rejects_invalid_prog_binding_shapes() {
    for source in [
        "(prog)",
        "(prog 1)",
        "(prog ((x 1 2)))",
        "(prog ((1 1)))",
        "(prog ((x 1) (x 2)))",
        "(prog* ((x 1) (x 2)))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_dolist_accumulation_empty_list_and_final_binding() {
    assert_eq!(
        evaluate(
            "(let ((total 0))
               (dolist (item '(1 2 3) (list total item))
                 (setq total (+ total item))))"
        )
        .to_string(),
        "(6 NIL)"
    );
    assert_eq!(
        evaluate("(dolist (item nil item) (setq item 1))").to_string(),
        "NIL"
    );
}

#[test]
fn rejects_invalid_destructuring_bind_shapes_and_arguments() {
    for source in [
        "(destructuring-bind (first) 1 first)",
        "(destructuring-bind (first second) (list 1) first)",
        "(destructuring-bind (first) (list 1 2) first)",
        "(destructuring-bind ((first second)) (list 1) first)",
        "(destructuring-bind (first &key key) (list 1 :key) key)",
        "(destructuring-bind (first &key key) (list 1 :key 2 :extra 3) key)",
        "(destructuring-bind (first &key key) (list 1 2) key)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_prog1_and_prog2_in_order_and_returns_retained_value() {
    assert_eq!(
        evaluate(
            "(let ((events nil))
               (list
                 (prog1
                   (progn (setq events (cons :first events)) 10)
                   (setq events (cons :second events))
                   (setq events (cons :third events)))
                 events))"
        )
        .to_string(),
        "(10 (:THIRD :SECOND :FIRST))"
    );
    assert_eq!(
        evaluate(
            "(let ((events nil))
               (list
                 (prog2
                   (progn (setq events (cons :first events)) 10)
                   (progn (setq events (cons :second events)) 20)
                   (setq events (cons :third events))
                   (setq events (cons :fourth events)))
                 events))"
        )
        .to_string(),
        "(20 (:FOURTH :THIRD :SECOND :FIRST))"
    );
}

#[test]
fn rejects_prog1_and_prog2_without_required_forms() {
    let prog1 = Runtime::new().eval_source("(prog1)").must_fail();
    assert!(matches!(
        prog1,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 0,
        } if function == "prog1" && expected == "at least one"
    ));

    let prog2 = Runtime::new().eval_source("(prog2 1)").must_fail();
    assert!(matches!(
        prog2,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 1,
        } if function == "prog2" && expected == "at least two"
    ));
}

#[test]
fn rejects_invalid_dotimes_and_dolist_forms() {
    let invalid_binding = Runtime::new().eval_source("(dotimes item)").must_fail();
    assert!(matches!(
        invalid_binding,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));

    let invalid_count = Runtime::new()
        .eval_source("(dotimes (index nil) index)")
        .must_fail();
    assert!(matches!(
        invalid_count,
        ncl_runtime::RuntimeError::Type { expected, .. } if expected == "INTEGER"
    ));

    let invalid_list = Runtime::new()
        .eval_source("(dolist (item 42) item)")
        .must_fail();
    assert!(matches!(
        invalid_list,
        ncl_runtime::RuntimeError::Type { expected, .. } if expected == "LIST"
    ));
}

#[test]
fn evaluates_tagbody_and_go_with_forward_and_backward_jumps() {
    let source = r"
        (let ((count 0))
          (tagbody
            start
            (setq count (+ count 1))
            (if (= count 3) (go done) (go start))
            done)
          count)
    ";

    assert_eq!(evaluate(source).to_string(), "3");
}

#[test]
fn propagates_unmatched_go_through_ignore_errors() {
    let error = Runtime::new()
        .eval_source("(ignore-errors (go missing))")
        .must_fail();

    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Go {
            tag,
            target: None,
            ..
        } if tag == "MISSING"
    ));
}

#[test]
fn rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)", "(go 1.5)"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
    assert!(Runtime::new().eval_source("(tagbody start start)").is_err());
}

#[test]
fn rejects_malformed_special_forms_at_their_boundaries() {
    for source in support::MALFORMED_SPECIAL_FORMS {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }

    assert!(
        Runtime::new()
            .eval_source("(eval-when ((execute)) 1)")
            .is_err()
    );
    assert!(Runtime::new().eval_source("(let ((x 1)) `,@x)").is_err());
}

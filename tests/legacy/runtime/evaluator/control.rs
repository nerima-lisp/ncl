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
        "(destructuring-bind (first))",
        "(destructuring-bind (first &environment env) (list 1) first)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_destructuring_bind_arity_error_with_exact_shape() {
    let error = Runtime::new()
        .eval_source("(destructuring-bind (first))")
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 1,
        } if function == "destructuring-bind" && expected == "two or more"
    ));
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

    let invalid_dolist_binding = Runtime::new().eval_source("(dolist item item)").must_fail();
    assert!(matches!(
        invalid_dolist_binding,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));
}

#[test]
fn rejects_invalid_do_and_do_star_shapes_and_arities() {
    let do_arity = Runtime::new().eval_source("(do)").must_fail();
    assert!(matches!(
        do_arity,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 0,
        } if function == "do" && expected == "at least two"
    ));

    let do_star_arity = Runtime::new().eval_source("(do* ((x 1)))").must_fail();
    assert!(matches!(
        do_star_arity,
        ncl_runtime::RuntimeError::Arity {
            function,
            expected,
            actual: 1,
        } if function == "do*" && expected == "at least two"
    ));

    for source in [
        "(do x (t) x)",
        "(do ((x 1)) t x)",
        "(do ((x 1)) () x)",
        "(do (x) (t) x)",
        "(do ((x 1 2 3)) (t) x)",
        "(do ((x 1) (x 2)) (t) x)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_do_with_parallel_stepping_and_do_star_with_sequential_stepping() {
    assert_eq!(
        evaluate(
            "(do ((temp-one 1 temp-two)
                  (temp-two 1 (+ temp-one temp-two))
                  (temp-three 0 (1+ temp-three)))
                 ((= 10 temp-three) temp-one))"
        )
        .to_string(),
        "89"
    );
    assert_eq!(
        evaluate(
            "(do* ((x 1 (+ x 1)) (y x (+ y x)))
                  ((> x 3) y))"
        )
        .to_string(),
        "10"
    );
    assert_eq!(
        evaluate("(do ((|counter| 0 (+ |counter| 1))) ((= |counter| 3) |counter|))").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(do ((x 0 (+ x 1)) (y 5)) ((= x 3) y))").to_string(),
        "5"
    );
}

#[test]
fn do_return_short_circuits_both_initialization_and_iteration() {
    assert_eq!(evaluate("(do ((x (return 42))) (t) x)").to_string(), "42");
    assert_eq!(
        evaluate("(do ((x 0 (+ x 1))) ((= x 3) :not-reached) (when (= x 1) (return :stopped)))")
            .to_string(),
        ":STOPPED"
    );
}

#[test]
fn do_propagates_ordinary_errors_from_initialization_and_body() {
    assert!(
        Runtime::new()
            .eval_source(r#"(do ((x (error "boom"))) (t) x)"#)
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source(r#"(do ((x 0)) (nil) (error "boom"))"#)
            .is_err()
    );
}

#[test]
fn rejects_invalid_let_flet_macrolet_and_symbol_macrolet_binding_shapes() {
    for source in [
        "(let (1) 1)",
        "(flet (x) x)",
        "(flet ((f () 1) (f () 2)) (f))",
        "(macrolet (x) x)",
        "(macrolet ((m () 1) (m () 2)) (m))",
        "(symbol-macrolet (x) x)",
        "(symbol-macrolet ((a 1) (a 2)) a)",
        "(define-symbol-macro x)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
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

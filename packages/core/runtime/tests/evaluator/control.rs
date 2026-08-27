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
fn evaluates_do_and_do_star_with_parallel_and_sequential_bindings() {
    assert_eq!(
        evaluate(
            "(list
               (let ((i 9))
                 (do ((i 1) (j i)) ((= i 1) j)))
               (let ((i 9))
                 (do* ((i 1) (j i)) ((= i 1) j)))
               (do ((i 0 (1+ i)) (j 0 i)) ((= i 3) j))
               (do* ((i 0 (1+ i)) (j 0 i)) ((= i 3) j)))",
        )
        .to_string(),
        "(9 1 2 3)"
    );
}

#[test]
fn evaluates_do_with_implicit_block_and_tagbody() {
    assert_eq!(
        evaluate(
            "(do ((i 0 (1+ i)))
                 ((= i 3) -1)
               (if (= i 2) (go finished) (go next))
               finished
               (return-from nil 42)
               next)"
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_prog_and_prog_star_with_parallel_and_sequential_bindings() {
    assert_eq!(
        evaluate(
            "(list
               (let ((i 9))
                 (prog ((i 1) (j i)) (return-from nil (list i j))))
               (let ((i 9))
                 (prog* ((i 1) (j i)) (return-from nil (list i j))))
               (prog () 42))",
        )
        .to_string(),
        "((1 9) (1 1) NIL)"
    );
}

#[test]
fn evaluates_prog_with_implicit_block_and_tagbody() {
    assert_eq!(
        evaluate(
            "(prog ((i 0))
               start
               (setq i (1+ i))
               (if (= i 2) (return-from nil i) (go start)))",
        )
        .to_string(),
        "2"
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
fn evaluates_return_as_an_implicit_nil_block_exit() {
    assert_eq!(
        evaluate(
            "(prog ((value 1))
               (return (+ value 41))
               (setq value 0))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(evaluate("(prog () (return))").to_string(), "NIL");
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
fn evaluates_destructuring_bind_with_nested_and_dotted_patterns() {
    assert_eq!(
        evaluate(
            "(destructuring-bind (first (second third)) (list 1 (list 2 3))
               (+ first (+ second third)))"
        )
        .to_string(),
        "6"
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (head . tail) (list 1 2 3)
               (+ head (car tail)))"
        )
        .to_string(),
        "3"
    );
}

#[test]
fn evaluates_destructuring_bind_lambda_list_parameters() {
    assert_eq!(
        evaluate(
            "(destructuring-bind (&whole whole (first second) &optional third)
               (list (list 1 2) 3)
               (list whole first second third))"
        )
        .to_string(),
        "(((1 2) 3) 1 2 3)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                  &key (scale 2 scale-p)
                                  &allow-other-keys
                                  &aux (total (+ first second scale)))
               (list 3 :scale 4 :ignored 9)
               (list first second second-p scale scale-p total))",
        )
        .to_string(),
        "(3 4 NIL 4 T 11)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &optional (second (+ first 1) second-p)
                                  &key (scale 2 scale-p))
               (list 3 5 :scale 6)
               (list first second second-p scale scale-p))",
        )
        .to_string(),
        "(3 5 T 6 T)",
    );
    assert_eq!(
        evaluate(
            "(destructuring-bind (first &rest rest &aux (count (length rest)))
               (list 3 4 5)
               (list first rest count))",
        )
        .to_string(),
        "(3 (4 5) 2)",
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
fn tagbody_returns_nil_and_does_not_evaluate_labels() {
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

#[test]
fn supports_integer_and_keyword_tags() {
    let source = r"
        (let ((count 0))
          (tagbody
            10
            (setq count (+ count 1))
            (if (= count 2) (go :done) (go 10))
            :done)
          count)
    ";

    assert_eq!(evaluate(source).to_string(), "2");
}

#[test]
fn captures_an_active_tagbody_target_in_a_closure() {
    let source = r"
        (let ((value 0))
          (tagbody
            start
            (setq value 1)
            (funcall (lambda () (go done)))
            (setq value 99)
            done)
          value)
    ";

    assert_eq!(evaluate(source).to_string(), "1");
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

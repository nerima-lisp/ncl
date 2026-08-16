use super::{Runtime, RuntimeError, evaluate};

#[test]
fn compiled_evaluates_rational_literals_and_exact_arithmetic() {
    assert_eq!(
        evaluate(
            "(list 1/2 2/4 (+ 1/2 1/3) (- 3/2 1/2) (* 2/3 9/4) (/ 2/3 4/5) (+ 1 1/2) (= 1 2/2) (< 1/3 1/2) (rationalp 1/2) (rationalp 1) (typep 1/2 'ratio) (typep 1/2 'rational) (numberp 1/2) (floatp 1/2))"
        )
        .to_string(),
          "(1/2 1/2 5/6 1 3/2 5/6 3/2 T T T T T T T NIL)"
    );
}

#[test]
fn compiled_tagbody_and_go_with_forward_and_backward_jumps() {
    let source = r#"
        (let ((count 0))
          (tagbody
            start
            (setq count (+ count 1))
            (if (= count 3) (go done) (go start))
            done)
          count)
    "#;

    assert_eq!(evaluate(source).to_string(), "3");
}

#[test]
fn compiled_unmatched_go_is_not_swallowed_by_ignore_errors() {
    let error = Runtime::new()
        .eval_compiled_source("(ignore-errors (go missing))")
        .unwrap_err();

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
fn compiled_tagbody_returns_nil_and_does_not_evaluate_labels() {
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

#[test]
fn compiled_supports_integer_and_keyword_tags() {
    let source = r#"
        (let ((count 0))
          (tagbody
            10
            (setq count (+ count 1))
            (if (= count 2) (go :done) (go 10))
            :done)
          count)
    "#;

    assert_eq!(evaluate(source).to_string(), "2");
}

#[test]
fn compiled_captures_an_active_tagbody_target_in_a_closure() {
    let source = r#"
        (let ((value 0))
          (tagbody
            start
            (setq value 1)
            (funcall (lambda () (go done)))
            (setq value 99)
            done)
          value)
    "#;

    assert_eq!(evaluate(source).to_string(), "1");
}

#[test]
fn compiled_rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)"] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }

    let invalid_tag = Runtime::new().eval_compiled_source("(go 1.5)").unwrap_err();
    assert!(
        matches!(
            invalid_tag,
            RuntimeError::InvalidForm { ref message, .. }
                if message == "go tag must be a symbol or integer"
        ),
        "got: {invalid_tag:?}"
    );

    let duplicate_tag = Runtime::new()
        .eval_compiled_source("(tagbody start start)")
        .unwrap_err();
    assert!(
        matches!(
            duplicate_tag,
            RuntimeError::InvalidForm { ref message, .. }
                if message == "tagbody contains duplicate tag"
        ),
        "got: {duplicate_tag:?}"
    );
}

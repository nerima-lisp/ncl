use super::{Runtime, evaluate};

#[test]
fn evaluates_tagbody_and_go_with_forward_and_backward_jumps() {
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
fn propagates_unmatched_go_through_ignore_errors() {
    let error = Runtime::new()
        .eval_source("(ignore-errors (go missing))")
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
fn tagbody_returns_nil_and_does_not_evaluate_labels() {
    assert_eq!(
        evaluate("(list (tagbody start done) 42)").to_string(),
        "(NIL 42)"
    );
}

#[test]
fn supports_integer_and_keyword_tags() {
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
fn captures_an_active_tagbody_target_in_a_closure() {
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
fn rejects_invalid_go_shapes_and_tags() {
    for source in ["(go)", "(go missing extra)", "(go 1.5)"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
    assert!(Runtime::new().eval_source("(tagbody start start)").is_err());
}

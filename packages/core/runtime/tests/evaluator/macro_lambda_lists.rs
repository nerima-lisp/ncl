use super::*;

#[test]
fn evaluates_macro_key_parameter_with_explicit_keyword_designator_list() {
    assert_eq!(
        evaluate("(macrolet ((f (&key ((:kw var) 9)) var)) (list (f) (f :kw 5)))").to_string(),
        "(9 5)"
    );
}

#[test]
fn rejects_malformed_macro_keyword_parameter_shapes() {
    for source in [
        "(destructuring-bind (&key ((:kw var extra) 1)) nil var)",
        "(destructuring-bind (&key ((not-a-keyword var) 1)) nil var)",
        "(destructuring-bind (&key ((: var) 1)) nil var)",
        "(destructuring-bind (&key (: var)) nil var)",
        "(destructuring-bind (&key (:kw)) nil 1)",
        "(destructuring-bind (&key ((a . b) 1)) nil a)",
        "(destructuring-bind (&key #\\a) nil 1)",
        "(destructuring-bind (&key (var 1 supplied extra)) nil var)",
        "(destructuring-bind (&key (a 1) (a 2)) nil a)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_malformed_macro_optional_and_auxiliary_parameter_shapes() {
    for source in [
        "(destructuring-bind (&optional #\\a) nil 1)",
        "(destructuring-bind (&aux #\\a) nil 1)",
        "(destructuring-bind (&aux (\"bad\" 1)) nil 1)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_a_top_level_macro_destructuring_pattern_that_is_not_a_symbol_or_list() {
    let error = Runtime::new()
        .eval_source(r#"(destructuring-bind "x" (list 1) 1)"#)
        .must_fail();
    assert!(matches!(
        error,
        ncl_runtime::RuntimeError::InvalidForm { .. }
    ));
}

#[test]
fn rejects_an_empty_list_as_a_macro_keyword_parameter_instead_of_panicking() {
    // FR-012 regression: parse_macro_keyword_parameter's `FormKind::List(_)`
    // arm (an empty-list keyword-parameter spec) used to fall through to a
    // bare `unreachable!()` and crash the process. Both call paths that
    // reach it -- destructuring-bind and defmacro -- must now report a
    // typed error instead.
    let destructuring_error = Runtime::new()
        .eval_source("(destructuring-bind (&key ()) nil 1)")
        .must_fail();
    assert!(matches!(
        destructuring_error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("must not be empty")
    ));

    let defmacro_error = Runtime::new()
        .eval_source("(defmacro m (&key ()) 1)")
        .must_fail();
    assert!(matches!(
        defmacro_error,
        ncl_runtime::RuntimeError::InvalidForm { message, .. }
            if message.contains("must not be empty")
    ));
}

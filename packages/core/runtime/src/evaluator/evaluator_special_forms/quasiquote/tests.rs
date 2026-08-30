use crate::Runtime;

#[test]
fn quasiquote_propagates_errors_from_nested_unquotes_splices_and_vectors() {
    for source in [
        "(quasiquote (a (quasiquote (unquote (unquote (car 5))))))",
        "(quasiquote #((unquote (car 5))))",
        "(quasiquote ((unquote (car 5)) . b))",
        "(quasiquote (a . (unquote-splicing (car 5))))",
        "(quasiquote (1 (unquote-splicing (car 5))))",
        "(quasiquote (a . (unquote (car 5))))",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn quasiquote_rejects_unquote_splicing_a_non_list_value() {
    let error = Runtime::new()
        .eval_source("(quasiquote (1 (unquote-splicing 5)))")
        .map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
    assert!(matches!(
        error,
        crate::RuntimeError::InvalidForm { message, .. }
            if message == "unquote-splicing requires a proper list"
    ));
}

use super::*;

#[test]
fn parse_do_form_rejects_a_non_list_binding_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("DO", span),
        Form::atom("BINDINGS", span),
        Form::list(vec![Form::atom("T", span)], span),
    ];
    let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
        |error| error,
        |value| panic!("bindings must be a list, got {value:?}"),
    );
    assert!(
        matches!(error.kind, CompileErrorKind::ExpectedList { context } if context == "DO bindings")
    );
}

#[test]
fn parse_do_form_rejects_a_binding_with_wrong_arity() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("DO", span),
        Form::list(vec![Form::list(Vec::new(), span)], span),
        Form::list(vec![Form::atom("T", span)], span),
    ];
    let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
        |error| error,
        |value| panic!("an empty binding has no name, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn parse_do_form_rejects_duplicate_binding_names() {
    let span = Span::new(0, 1);
    let bindings = Form::list(
        vec![
            Form::list(vec![Form::atom("X", span)], span),
            Form::list(vec![Form::atom("X", span)], span),
        ],
        span,
    );
    let items = vec![
        Form::atom("DO", span),
        bindings,
        Form::list(vec![Form::atom("T", span)], span),
    ];
    let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
        |error| error,
        |value| panic!("duplicate DO binding names must be rejected, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
}

#[test]
fn parse_do_form_propagates_an_invalid_binding_name_error() {
    let span = Span::new(0, 1);
    let bindings = Form::list(vec![Form::list(vec![Form::atom(":x", span)], span)], span);
    let items = vec![
        Form::atom("DO", span),
        bindings,
        Form::list(vec![Form::atom("T", span)], span),
    ];
    let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
        |error| error,
        |value| panic!("a keyword cannot name a DO binding, got {value:?}"),
    );
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

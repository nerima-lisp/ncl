use super::*;

fn dotted(span: Span) -> Form {
    Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
}

#[test]
fn compile_handler_bind_propagates_a_handler_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(vec![Form::atom("MY-CONDITION", span), dotted(span)], span);
    let items = vec![
        Form::atom("HANDLER-BIND", span),
        Form::list(vec![clause], span),
    ];

    let error = state
        .compile_handler_bind(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed handler body should fail to compile, got {value:?}"),
        );

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_handler_bind_propagates_a_protected_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-CONDITION", span), Form::atom("F", span)],
        span,
    );
    let items = vec![
        Form::atom("HANDLER-BIND", span),
        Form::list(vec![clause], span),
        dotted(span),
    ];

    let error = state
        .compile_handler_bind(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed protected body form should fail to compile, got {value:?}"),
        );

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_handler_bind_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-CONDITION", span), Form::atom("F", span)],
        span,
    );
    let items = vec![
        Form::atom("HANDLER-BIND", span),
        Form::list(vec![clause], span),
    ];

    let error = state.compile_handler_bind(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

use super::*;

fn dotted(span: Span) -> Form {
    Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
}

#[test]
fn compile_restart_bind_propagates_a_binding_function_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(vec![Form::atom("MY-RESTART", span), dotted(span)], span);
    let items = vec![
        Form::atom("RESTART-BIND", span),
        Form::list(vec![clause], span),
    ];
    let error = state
        .compile_restart_bind(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed restart function should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_restart_bind_propagates_a_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-RESTART", span), Form::atom("F", span)],
        span,
    );
    let items = vec![
        Form::atom("RESTART-BIND", span),
        Form::list(vec![clause], span),
        dotted(span),
    ];
    let error = state
        .compile_restart_bind(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed body form should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_restart_bind_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-RESTART", span), Form::atom("F", span)],
        span,
    );
    let items = vec![
        Form::atom("RESTART-BIND", span),
        Form::list(vec![clause], span),
    ];
    let error = state.compile_restart_bind(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_with_simple_restart_propagates_a_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-RESTART", span), Form::atom("REPORT", span)],
        span,
    );
    let items = vec![
        Form::atom("WITH-SIMPLE-RESTART", span),
        clause,
        dotted(span),
    ];
    let error = state
        .compile_with_simple_restart(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed body form should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_with_simple_restart_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![Form::atom("MY-RESTART", span), Form::atom("REPORT", span)],
        span,
    );
    let items = vec![Form::atom("WITH-SIMPLE-RESTART", span), clause];
    let error = state
        .compile_with_simple_restart(99, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

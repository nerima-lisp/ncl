use super::*;

fn dotted(span: Span) -> Form {
    Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
}

#[test]
fn compile_ignore_errors_propagates_a_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![Form::atom("IGNORE-ERRORS", span), dotted(span)];
    let error = state
        .compile_ignore_errors(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed body should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_ignore_errors_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![Form::atom("IGNORE-ERRORS", span), Form::atom("1", span)];
    let error = state.compile_ignore_errors(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_handler_case_propagates_a_protected_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![
            Form::atom("MY-CONDITION", span),
            Form::list(Vec::new(), span),
        ],
        span,
    );
    let items = vec![Form::atom("HANDLER-CASE", span), dotted(span), clause];
    let error = state
        .compile_handler_case(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed protected form should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_handler_case_propagates_an_invalid_variable_name_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![
            Form::atom("MY-CONDITION", span),
            Form::list(vec![Form::atom(":x", span)], span),
        ],
        span,
    );
    let items = vec![
        Form::atom("HANDLER-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let error = state
        .compile_handler_case(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a keyword cannot be a handler-case variable name, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_handler_case_propagates_a_clause_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![
            Form::atom("MY-CONDITION", span),
            Form::list(Vec::new(), span),
            dotted(span),
        ],
        span,
    );
    let items = vec![
        Form::atom("HANDLER-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let error = state
        .compile_handler_case(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed clause body should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_handler_case_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![
            Form::atom("MY-CONDITION", span),
            Form::list(Vec::new(), span),
        ],
        span,
    );
    let items = vec![
        Form::atom("HANDLER-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let error = state.compile_handler_case(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

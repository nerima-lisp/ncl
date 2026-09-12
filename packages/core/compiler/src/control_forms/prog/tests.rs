use super::*;

fn dotted(span: Span) -> Form {
    Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
}

#[test]
fn compile_prog_propagates_a_sequential_binding_initializer_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let bindings = Form::list(
        vec![Form::list(vec![Form::atom("X", span), dotted(span)], span)],
        span,
    );
    let items = vec![Form::atom("PROG*", span), bindings];
    let error = state
        .compile_prog(function, span, &items, true)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed binding initializer should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_prog_propagates_a_parallel_binding_initializer_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let bindings = Form::list(
        vec![Form::list(vec![Form::atom("X", span), dotted(span)], span)],
        span,
    );
    let items = vec![Form::atom("PROG", span), bindings];
    let error = state
        .compile_prog(function, span, &items, false)
        .map_or_else(
            |error| error,
            |value| panic!("a malformed binding initializer should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_prog_propagates_a_body_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG", span),
        Form::list(Vec::new(), span),
        dotted(span),
    ];
    let error = state
        .compile_prog(function, span, &items, false)
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
fn compile_prog_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![Form::atom("PROG", span), Form::list(Vec::new(), span)];
    let error = state.compile_prog(99, span, &items, false).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn parse_prog_bindings_rejects_a_non_symbol_non_list_binding() {
    let span = Span::new(0, 1);
    let error =
        CompileState::parse_prog_bindings(&[Form::new(FormKind::String("bad".to_string()), span)])
            .map_or_else(
                |error| error,
                |value| panic!("a literal cannot name a PROG binding, got {value:?}"),
            );
    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn parse_prog_bindings_tracks_escaped_names_separately_from_normalized_ones() {
    let span = Span::new(0, 1);
    let parsed = CompileState::parse_prog_bindings(&[
        Form::list(vec![Form::atom("|X|", span)], span),
        Form::list(vec![Form::atom("x", span)], span),
    ])
    .unwrap_or_else(|error| {
        panic!("an escaped name and its normalized form do not collide: {error}")
    });
    assert_eq!(parsed.len(), 2);
    assert!(parsed[0].1);
    assert!(!parsed[1].1);
}

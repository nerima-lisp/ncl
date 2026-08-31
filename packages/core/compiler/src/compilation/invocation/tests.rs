use super::*;

fn dotted(span: Span) -> Form {
    Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
}

#[test]
fn compile_funcall_propagates_an_argument_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![Form::atom("FUNCALL", span), dotted(span)];
    let error = state.compile_funcall(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed argument should fail to compile, got {value:?}"),
    );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_funcall_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![Form::atom("FUNCALL", span), Form::atom("F", span)];
    let error = state.compile_funcall(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_eval_propagates_an_argument_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![Form::atom("EVAL", span), dotted(span)];
    let error = state.compile_eval(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed argument should fail to compile, got {value:?}"),
    );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_apply_propagates_an_argument_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("APPLY", span),
        Form::atom("F", span),
        dotted(span),
    ];
    let error = state.compile_apply(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed argument should fail to compile, got {value:?}"),
    );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_mapcar_propagates_an_argument_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("MAPCAR", span),
        Form::atom("F", span),
        dotted(span),
    ];
    let error = state
        .compile_list_mapping(function, span, &items, "MAPCAR")
        .map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_map_into_propagates_an_argument_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("MAP-INTO", span),
        Form::atom("D", span),
        dotted(span),
    ];
    let error = state.compile_map_into(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed argument should fail to compile, got {value:?}"),
    );
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedForm { .. }
    ));
}

#[test]
fn compile_map_into_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("MAP-INTO", span),
        Form::atom("D", span),
        Form::atom("S", span),
    ];
    let error = state.compile_map_into(99, span, &items).map_or_else(
        |error| error,
        |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
    );
    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

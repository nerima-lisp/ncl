use super::*;
use ncl_syntax::read;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms =
        read(source).unwrap_or_else(|error| panic!("test source should parse: {error}"));
    let form = forms.remove(0);
    let FormKind::List(items) = form.kind else {
        panic!("expected a list form, got {form:?}");
    };
    items
}

fn expect_nested_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_flet_rejects_a_non_symbol_local_function_name() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((5 () 1)) 1)");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(
            |error| error,
            |value| panic!("a numeric literal is not a valid local function name, got {value:?}"),
        );

    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_flet_propagates_a_malformed_parameter_list_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f 5 3)) 1)");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(
            |error| error,
            |value| panic!("a non-list parameter form must be rejected, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::ExpectedList { .. }));
}

#[test]
fn compile_flet_propagates_an_optional_default_compile_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f (&optional (v (function))) v)) (f))");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a malformed &optional default value must propagate its own compile error, got {value:?}"));

    expect_nested_arity(&error);
}

#[test]
fn compile_flet_propagates_a_keyword_default_compile_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f (&key (v (function))) v)) (f))");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a malformed &key default value must propagate its own compile error, got {value:?}"));

    expect_nested_arity(&error);
}

#[test]
fn compile_flet_propagates_an_auxiliary_default_compile_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f (&aux (v (function))) v)) (f))");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a malformed &aux default value must propagate its own compile error, got {value:?}"));

    expect_nested_arity(&error);
}

#[test]
fn compile_flet_propagates_a_local_function_body_compile_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f () (function))) (f))");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a malformed local function body must propagate its own compile error, got {value:?}"));

    expect_nested_arity(&error);
}

#[test]
fn compile_flet_propagates_a_main_body_compile_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f () 1)) (function))");

    let error = state
        .compile_flet(function, span, &items, false)
        .map_or_else(
            |error| error,
            |value| {
                panic!("a malformed main body must propagate its own compile error, got {value:?}")
            },
        );

    expect_nested_arity(&error);
}

#[test]
fn compile_flet_labels_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(labels ((f () 1)) (f))");

    let error = state
        .compile_flet(function + 1, span, &items, true)
        .map_or_else(
            |error| error,
            |value| panic!("entering scope on an invalid function id must fail, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_flet_reports_an_internal_error_when_closing_over_an_invalid_function_id() {
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(flet ((f () 1)) (f))");

    let error = state
        .compile_flet(usize::MAX, span, &items, false)
        .map_or_else(|error| error, |value| panic!("making a closure over an invalid enclosing function id must fail, got {value:?}"));

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

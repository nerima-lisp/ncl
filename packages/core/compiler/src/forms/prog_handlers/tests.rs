use super::*;
use crate::CompileErrorKind;

fn bad_catch(span: Span) -> Form {
    Form::list(vec![Form::atom("CATCH", span)], span)
}

fn expect_internal(error: &CompileError) {
    assert!(
        matches!(error.kind, CompileErrorKind::Internal { .. }),
        "expected an internal error, got {:?}",
        error.kind
    );
}

fn expect_catch_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "CATCH"),
        other => panic!("expected the nested CATCH arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_prog1_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG1", span),
        Form::atom("1", span),
        Form::atom("2", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog1(function + 1, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("an out-of-range target function must fail entering the scope, got {value:?}")
        },
    );
    expect_internal(&error);
}

#[test]
fn compile_prog1_propagates_malformed_first_form() {
    let span = Span::new(0, 1);
    let items = vec![Form::atom("PROG1", span), bad_catch(span)];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog1(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed retained form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_prog1_propagates_malformed_tail_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG1", span),
        Form::atom("1", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog1(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed tail form must propagate its own compile error, got {value:?}"),
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_prog2_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG2", span),
        Form::atom("1", span),
        Form::atom("2", span),
        Form::atom("3", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog2(function + 1, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("an out-of-range target function must fail entering the scope, got {value:?}")
        },
    );
    expect_internal(&error);
}

#[test]
fn compile_prog2_propagates_malformed_first_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG2", span),
        bad_catch(span),
        Form::atom("2", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog2(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed first form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_prog2_propagates_malformed_second_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG2", span),
        Form::atom("1", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog2(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed retained form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_prog2_propagates_malformed_tail_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROG2", span),
        Form::atom("1", span),
        Form::atom("2", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_prog2(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed tail form must propagate its own compile error, got {value:?}"),
    );
    expect_catch_arity(&error);
}

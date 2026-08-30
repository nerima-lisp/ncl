use super::*;

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
fn compile_catch_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("CATCH", span),
        Form::atom("T", span),
        Form::atom("1", span),
    ];
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());

    let error = state.compile_catch(usize::MAX, span, &items).map_or_else(
        |error| error,
        |value| panic!("an out-of-range target function must fail the final emit, got {value:?}"),
    );
    expect_internal(&error);
}

#[test]
fn compile_throw_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("THROW", span),
        Form::atom("TAG", span),
        Form::atom("1", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_throw(function + 1, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("an out-of-range target function must fail compiling the tag, got {value:?}")
        },
    );
    expect_internal(&error);
}

#[test]
fn compile_throw_propagates_malformed_result_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("THROW", span),
        Form::atom("TAG", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_throw(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed result form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_progv_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROGV", span),
        Form::atom("SYMBOLS", span),
        Form::atom("VALUES", span),
        Form::atom("BODY", span),
    ];
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());

    let error = state.compile_progv(usize::MAX, span, &items).map_or_else(
        |error| error,
        |value| panic!("an out-of-range target function must fail the final emit, got {value:?}"),
    );
    expect_internal(&error);
}

#[test]
fn compile_progv_propagates_malformed_symbols_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROGV", span),
        bad_catch(span),
        Form::atom("VALUES", span),
        Form::atom("BODY", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_progv(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed symbols form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

mod progv_tests;
mod unwind_protect_tests;

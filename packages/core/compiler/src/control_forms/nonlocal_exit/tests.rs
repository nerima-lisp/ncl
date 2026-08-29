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

#[test]
fn compile_progv_propagates_malformed_values_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROGV", span),
        Form::atom("SYMBOLS", span),
        bad_catch(span),
        Form::atom("BODY", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_progv(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a malformed values form must propagate its own compile error, got {value:?}")
        },
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_progv_propagates_malformed_body_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("PROGV", span),
        Form::atom("SYMBOLS", span),
        Form::atom("VALUES", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_progv(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed body form must propagate its own compile error, got {value:?}"),
    );
    expect_catch_arity(&error);
}

#[test]
fn compile_unwind_protect_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("UNWIND-PROTECT", span),
        Form::atom("1", span),
        Form::atom("2", span),
    ];
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());

    let error = state
        .compile_unwind_protect(usize::MAX, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!("an out-of-range target function must fail the final emit, got {value:?}")
            },
        );
    expect_internal(&error);
}

#[test]
fn compile_unwind_protect_propagates_malformed_protected_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("UNWIND-PROTECT", span),
        bad_catch(span),
        Form::atom("2", span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_unwind_protect(function, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a malformed protected form must propagate its own compile error, got {value:?}"
                )
            },
        );
    expect_catch_arity(&error);
}

#[test]
fn compile_unwind_protect_propagates_malformed_cleanup_form() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("UNWIND-PROTECT", span),
        Form::atom("1", span),
        bad_catch(span),
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state
        .compile_unwind_protect(function, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a malformed cleanup form must propagate its own compile error, got {value:?}"
                )
            },
        );
    expect_catch_arity(&error);
}

use super::*;

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
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"));
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
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"));
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
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"));
    expect_catch_arity(&error);
}

use super::*;

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
    let error = state
        .compile_progv(function, span, &items)
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"));
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
    let error = state
        .compile_progv(function, span, &items)
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"));
    expect_catch_arity(&error);
}

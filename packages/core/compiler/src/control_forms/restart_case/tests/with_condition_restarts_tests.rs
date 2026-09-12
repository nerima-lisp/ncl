use super::*;

fn compile(items: &[Form], function: usize) -> CompileError {
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());
    state
        .compile_with_condition_restarts(function, Span::new(0, 1), items)
        .map_or_else(|error| error, |value| panic!("unexpected {value:?}"))
}

#[test]
fn rejects_invalid_function_id() {
    let s = Span::new(0, 1);
    expect_internal(&compile(
        &[
            Form::atom("WITH-CONDITION-RESTARTS", s),
            Form::atom("1", s),
            Form::atom("2", s),
            Form::atom("3", s),
        ],
        usize::MAX,
    ));
}
#[test]
fn propagates_malformed_condition_form() {
    let s = Span::new(0, 1);
    expect_catch_arity(&compile(
        &[
            Form::atom("WITH-CONDITION-RESTARTS", s),
            bad_catch(s),
            Form::atom("2", s),
            Form::atom("3", s),
        ],
        0,
    ));
}
#[test]
fn propagates_malformed_restarts_form() {
    let s = Span::new(0, 1);
    expect_catch_arity(&compile(
        &[
            Form::atom("WITH-CONDITION-RESTARTS", s),
            Form::atom("1", s),
            bad_catch(s),
            Form::atom("3", s),
        ],
        0,
    ));
}
#[test]
fn propagates_malformed_body_form() {
    let s = Span::new(0, 1);
    expect_catch_arity(&compile(
        &[
            Form::atom("WITH-CONDITION-RESTARTS", s),
            Form::atom("1", s),
            Form::atom("2", s),
            bad_catch(s),
        ],
        0,
    ));
}

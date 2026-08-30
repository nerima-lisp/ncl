use super::*;

#[test]
fn compile_restart_case_rejects_invalid_function_id() {
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        empty_clause(span),
    ];
    let mut state = CompileState::default();
    state.reserve_function(None, Vec::new());

    let error = state
        .compile_restart_case(usize::MAX, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!("an out-of-range target function must fail the final emit, got {value:?}")
            },
        );
    expect_internal(&error);
}

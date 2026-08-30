use super::*;

#[test]
fn compile_restart_case_propagates_malformed_clause_body() {
    let span = Span::new(0, 1);
    let clause = Form::list(
        vec![
            Form::atom("R", span),
            Form::list(Vec::new(), span),
            bad_catch(span),
        ],
        span,
    );
    let items = vec![
        Form::atom("RESTART-CASE", span),
        Form::atom("1", span),
        clause,
    ];
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());

    let error = state.compile_restart_case(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a malformed clause body form must propagate its own compile error, got {value:?}"),
    );
    expect_catch_arity(&error);
}

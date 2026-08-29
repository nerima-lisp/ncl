use super::*;

#[test]
fn compile_case_falls_back_to_case_operator_name_when_head_is_not_an_atom() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![Form::list(Vec::new(), span)];

    let Err(error) = state.compile_case(function, span, &items) else {
        panic!("a lone non-atom head still fails the arity check");
    };

    match error.kind {
        CompileErrorKind::Arity {
            operator,
            expected,
            actual,
        } => {
            assert_eq!(operator, "CASE");
            assert_eq!(expected, "at least one");
            assert_eq!(actual, 0);
        }
        other => panic!("expected an arity error, got {other:?}"),
    }
}

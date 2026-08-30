use super::*;

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
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "making a closure over an invalid enclosing function id must fail, got {value:?}"
                )
            },
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

use super::*;
use ncl_syntax::read;

fn parse_items(source: &str) -> Vec<Form> {
    let mut forms =
        read(source).unwrap_or_else(|error| panic!("test source should parse: {error}"));
    let form = forms.remove(0);
    let FormKind::List(items) = form.kind else {
        panic!("expected a list form, got {form:?}");
    };
    items
}

fn expect_nested_arity(error: &CompileError) {
    match &error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_setq_rejects_a_non_symbol_target() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setq 5 1)");

    let error = state.compile_setq(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a numeric literal is not a valid setq target, got {value:?}"),
    );

    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_setq_propagates_a_malformed_value_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(setq x (function))");

    let error = state.compile_setq(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a value form that fails to compile must propagate its own error, got {value:?}")
        },
    );

    expect_nested_arity(&error);
}

#[test]
fn compile_psetq_rejects_a_non_symbol_target() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(psetq 5 1 y 2)");

    let error = state.compile_psetq(function, span, &items).map_or_else(
        |error| error,
        |value| panic!("a numeric literal is not a valid psetq target, got {value:?}"),
    );

    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_psetq_propagates_a_malformed_value_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(psetq x (function) y 2)");

    let error = state.compile_psetq(function, span, &items).map_or_else(
        |error| error,
        |value| {
            panic!("a value form that fails to compile must propagate its own error, got {value:?}")
        },
    );

    expect_nested_arity(&error);
}

#[test]
fn compile_psetq_uses_psetq_exact_for_an_escaped_name() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(psetq |Mixed| 1)");

    state
        .compile_psetq(function, span, &items)
        .unwrap_or_else(|error| panic!("an escaped psetq target should still compile: {error}"));

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                Instruction::PsetqExact(names) if names == &[("Mixed".to_string(), true)]
            )),
        "escaped psetq target should bind with PsetqExact, got {:?}",
        state.functions[function].instructions
    );
}

#[test]
fn compile_multiple_value_setq_rejects_non_list_variables() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(multiple-value-setq x (values 1))");

    let error = state
        .compile_multiple_value_setq(function, span, &items)
        .map_or_else(
            |error| error,
            |value| panic!("a non-list variable form must be rejected, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::ExpectedList { .. }));
}

#[test]
fn compile_multiple_value_setq_rejects_a_non_symbol_variable() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(multiple-value-setq (5) (values 1))");

    let error = state
        .compile_multiple_value_setq(function, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a numeric literal is not a valid multiple-value-setq variable, got {value:?}"
                )
            },
        );

    assert!(matches!(
        error.kind,
        CompileErrorKind::ExpectedSymbol { .. }
    ));
}

#[test]
fn compile_multiple_value_setq_propagates_a_malformed_value_form_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(multiple-value-setq (x) (function))");

    let error = state
        .compile_multiple_value_setq(function, span, &items)
        .map_or_else(
            |error| error,
            |value| {
                panic!(
                    "a value form that fails to compile must propagate its own error, got {value:?}"
                )
            },
        );

    expect_nested_arity(&error);
}

#[test]
fn compile_multiple_value_setq_uses_exact_instruction_for_an_escaped_name() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(multiple-value-setq (|Mixed|) (values 1))");

    state
        .compile_multiple_value_setq(function, span, &items)
        .unwrap_or_else(|error| {
            panic!("an escaped multiple-value-setq variable should still compile: {error}")
        });

    assert!(
        state.functions[function]
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                Instruction::MultipleValueSetqExact(names) if names == &[("Mixed".to_string(), true)]
            )),
        "escaped multiple-value-setq variable should bind with MultipleValueSetqExact, got {:?}",
        state.functions[function].instructions
    );
}

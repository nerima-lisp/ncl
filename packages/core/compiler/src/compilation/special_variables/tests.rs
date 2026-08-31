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

#[test]
fn compile_defvar_defparameter_uses_define_special_exact_for_an_escaped_name() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(defparameter |Mixed| 1)");

    state
        .compile_defvar(function, span, &items, true)
        .unwrap_or_else(|error| {
            panic!("an escaped defparameter name should still compile: {error}")
        });

    assert!(
        state.functions[function]
            .instructions
            .contains(&Instruction::DefineSpecialExact {
                name: "Mixed".to_string(),
                force: true,
            }),
        "escaped DEFPARAMETER name should bind with DefineSpecialExact, got {:?}",
        state.functions[function].instructions
    );
}

#[test]
fn compile_defvar_registers_the_name_for_later_let_bindings() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);

    state
        .compile_defvar(function, span, &parse_items("(defvar *x* 1)"), false)
        .unwrap_or_else(|error| panic!("defvar should compile: {error}"));

    state
        .compile_let(function, span, &parse_items("(let ((*x* 2)) *x*)"), false)
        .unwrap_or_else(|error| panic!("later let should compile: {error}"));

    assert!(state.functions[function]
        .instructions
        .contains(&Instruction::DefineDynamicSpecial("*X*".to_string())));
}

#[test]
fn compile_defvar_defparameter_propagates_a_malformed_initializer_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(defparameter x (function))");

    let error = state
        .compile_defvar(function, span, &items, true)
        .map_or_else(|error| error, |value| panic!("a defparameter initializer that fails to compile must propagate its error, got {value:?}"));

    match error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_defvar_defparameter_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(defparameter x)");

    let error = state
        .compile_defvar(function + 1, span, &items, true)
        .map_or_else(
            |error| error,
            |value| panic!("emitting into an invalid function id must fail, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

#[test]
fn compile_defvar_propagates_a_malformed_initializer_error() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(defvar x (function))");

    let error = state
        .compile_defvar(function, span, &items, false)
        .map_or_else(|error| error, |value| panic!("a defvar initializer that fails to compile must propagate its error, got {value:?}"));

    match error.kind {
        CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "FUNCTION"),
        other => panic!("expected the nested FUNCTION arity error to propagate, got {other:?}"),
    }
}

#[test]
fn compile_defvar_reports_an_internal_error_for_an_invalid_function_id() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = parse_items("(defvar x)");

    let error = state
        .compile_defvar(function + 1, span, &items, false)
        .map_or_else(
            |error| error,
            |value| panic!("emitting into an invalid function id must fail, got {value:?}"),
        );

    assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
}

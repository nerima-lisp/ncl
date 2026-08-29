use super::*;

#[test]
fn compile_function_compiles_a_non_symbol_argument_as_an_expression() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("FUNCTION", span),
        Form::new(FormKind::String("ignored".to_string()), span),
    ];

    let Ok(()) = state.compile_function(function, span, &items) else {
        panic!("a non-symbol FUNCTION argument compiles as an ordinary expression");
    };

    assert_eq!(
        state.functions[function].instructions,
        vec![Instruction::Constant(Constant::String(
            "ignored".to_string()
        ))]
    );
}

#[test]
fn compile_defun_defines_an_escaped_name_with_exact_instructions() {
    let mut state = CompileState::default();
    let function = state.reserve_function(None, Vec::new());
    let span = Span::new(0, 1);
    let items = vec![
        Form::atom("DEFUN", span),
        Form::atom("|Mixed|", span),
        Form::list(Vec::new(), span),
        Form::atom("1", span),
    ];

    let Ok(()) = state.compile_defun(function, span, &items) else {
        panic!("an escaped defun name should still compile");
    };

    let instructions = &state.functions[function].instructions;
    assert!(
        instructions.contains(&Instruction::DefineExact("Mixed".to_string())),
        "escaped defun name should bind with DefineExact, got {instructions:?}"
    );
    assert!(
        instructions.contains(&Instruction::Constant(Constant::SymbolExact(
            "Mixed".to_string()
        ))),
        "escaped defun name should yield a SymbolExact constant, got {instructions:?}"
    );
}

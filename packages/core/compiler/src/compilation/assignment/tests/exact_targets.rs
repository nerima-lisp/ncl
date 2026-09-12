use super::*;

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

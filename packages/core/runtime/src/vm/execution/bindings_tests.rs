#[allow(clippy::wildcard_imports)]
use super::*;

#[test]
fn load_instructions_reject_unbound_variables() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);

    for instruction in [
        Instruction::Load("missing".to_string()),
        Instruction::LoadExact("Missing".to_string()),
    ] {
        let mut stack = Vec::new();
        let mut program_counter = 0;
        let result = execute_load_instruction(
            &runtime,
            &instruction,
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        );
        assert!(matches!(
            result,
            Err(RuntimeError::UnboundVariable { name, .. }) if name.eq_ignore_ascii_case("missing")
        ));
        assert_eq!(program_counter, 0);
    }
}

#[test]
fn define_exact_binds_the_escaped_name_and_returns_the_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut stack = vec![Value::Integer(7)];
    let mut program_counter = 0;

    let result = execute_definition_instruction(
        &runtime,
        &Instruction::DefineExact("Foo".to_string()),
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 1);
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(7)));
    assert!(matches!(
        environment.lookup_exact("Foo"),
        Some(Value::Integer(7))
    ));
    assert!(environment.lookup_exact("foo").is_none());
}

#[test]
fn define_special_exact_rejects_forced_redefinition_of_a_constant() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    runtime.define_constant_value_exact("LIMIT", Value::Integer(1));

    let mut stack = vec![Value::Integer(2)];
    let mut program_counter = 0;
    let result = execute_definition_instruction(
        &runtime,
        &Instruction::DefineSpecialExact {
            name: "LIMIT".to_string(),
            force: true,
        },
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == "cannot modify constant LIMIT"
    ));
    assert_eq!(program_counter, 0);
    assert!(matches!(stack[0], Value::Integer(2)));
}

#[test]
fn define_values_exact_binds_the_full_multiple_values_container() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let value = Value::values(vec![Value::Integer(1), Value::Integer(2)]);
    let mut stack = vec![value.clone()];
    let mut program_counter = 0;

    let result = execute_definition_instruction(
        &runtime,
        &Instruction::DefineValuesExact("Both".to_string()),
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 1);
    let Some(bound) = environment.lookup_exact("Both") else {
        panic!("define-values-exact must bind the escaped name");
    };
    assert_eq!(bound.to_string(), value.to_string());
}

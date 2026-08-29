#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_stack_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    scopes: &mut Vec<(Environment, usize, usize)>,
    environment: &mut Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::EnterScope => {
            scopes.push((
                environment.clone(),
                runtime.dynamic_depth(),
                runtime.exact_dynamic_depth(),
            ));
            *environment = environment.child();
            *program_counter += 1;
            Ok(true)
        }
        Instruction::ExitScope => {
            let (parent, depth, exact_depth) = scopes
                .pop()
                .ok_or_else(|| invalid("scope exit has no matching scope", span))?;
            runtime.truncate_dynamic(depth);
            runtime.truncate_exact_dynamic(exact_depth);
            *environment = parent;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Pop => {
            pop_value(stack, span, "pop")?;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Dup => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("dup has no value on the stack", span))?;
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Primary => {
            let value = pop_value(stack, span, "primary value")?;
            stack.push(value.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Values(value_count) => {
            if stack.len() < *value_count {
                return Err(invalid("values has too few stack values", span));
            }
            let values = stack.split_off(stack.len() - *value_count);
            stack.push(Value::values(values));
            *program_counter += 1;
            Ok(true)
        }
        Instruction::MultipleValueList => {
            let value = pop_value(stack, span, "multiple-value-list")?;
            stack.push(Value::list(value.multiple_values()));
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    fn assert_invalid(result: Result<(), RuntimeError>, expected: &str) {
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. }) if message == expected
        ));
    }

    #[test]
    fn stack_instructions_reject_missing_values() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases: [(Instruction, &str); 4] = [
            (Instruction::ExitScope, "scope exit has no matching scope"),
            (Instruction::Pop, "pop has no value on the stack"),
            (Instruction::Dup, "dup has no value on the stack"),
            (Instruction::Values(1), "values has too few stack values"),
        ];

        for (instruction, expected) in cases {
            let mut stack = Vec::new();
            let mut scopes = Vec::new();
            let mut environment = environment.clone();
            let mut program_counter = 0;
            assert_invalid(
                execute_stack_instruction(
                    &runtime,
                    &instruction,
                    &mut stack,
                    &mut scopes,
                    &mut environment,
                    &mut program_counter,
                    span,
                )
                .map(|_| ()),
                expected,
            );
        }
    }
}

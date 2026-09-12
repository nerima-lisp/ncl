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
        Instruction::CheckNthValueIndex(_) | Instruction::NthValue => {
            execute_nth_value(instruction, stack, span)?;
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn nth_value_index(value: &Value, span: Span) -> Result<usize, RuntimeError> {
    match value {
        Value::Integer(index) if *index >= 0 => {
            usize::try_from(*index).map_err(|_| RuntimeError::NumericOverflow)
        }
        Value::Integer(_) => Err(invalid("nth-value index must be non-negative", span)),
        Value::BigInteger(_) => Err(RuntimeError::NumericOverflow),
        value => Err(RuntimeError::Type {
            expected: "INTEGER".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        }),
    }
}

fn execute_nth_value(
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if let Instruction::CheckNthValueIndex(index_span) = instruction {
        let value = stack
            .last()
            .ok_or_else(|| invalid("nth-value index has no value on the stack", span))?;
        nth_value_index(value, *index_span)?;
    } else {
        if stack.len() < 2 {
            return Err(invalid("nth-value has too few stack values", span));
        }
        let values = pop_value(stack, span, "nth-value producer")?;
        let index = nth_value_index(&pop_value(stack, span, "nth-value index")?, span)?;
        stack.push(match values {
            Value::Values(values) => values.get(index).cloned().unwrap_or(Value::Nil),
            value if index == 0 => value,
            _ => Value::Nil,
        });
    }
    Ok(())
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
        let cases: [(Instruction, &str); 6] = [
            (Instruction::ExitScope, "scope exit has no matching scope"),
            (Instruction::Pop, "pop has no value on the stack"),
            (Instruction::Dup, "dup has no value on the stack"),
            (Instruction::Values(1), "values has too few stack values"),
            (
                Instruction::CheckNthValueIndex(span),
                "nth-value index has no value on the stack",
            ),
            (Instruction::NthValue, "nth-value has too few stack values"),
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

    #[test]
    fn nth_value_rejects_missing_producer_and_unchecked_invalid_indices() {
        let span = Span::new(0, 1);
        assert_invalid(
            execute_nth_value(&Instruction::NthValue, &mut vec![Value::Integer(0)], span),
            "nth-value has too few stack values",
        );
        for instruction in [Instruction::CheckNthValueIndex(span), Instruction::NthValue] {
            for index in [
                Value::Integer(-1),
                Value::Nil,
                Value::BigInteger(std::rc::Rc::new(ibig::IBig::from(1))),
            ] {
                let mut stack = vec![index.clone()];
                if matches!(instruction, Instruction::NthValue) {
                    stack.push(Value::Integer(42));
                }
                let result = execute_nth_value(&instruction, &mut stack, span);
                match index {
                    Value::Integer(_) => {
                        assert_invalid(result, "nth-value index must be non-negative");
                    }
                    Value::BigInteger(_) => {
                        assert!(matches!(result, Err(RuntimeError::NumericOverflow)));
                    }
                    _ => assert!(matches!(
                        result,
                        Err(RuntimeError::Type { expected, span: Some(actual_span), .. })
                            if expected == "INTEGER" && actual_span == span
                    )),
                }
            }
        }
    }

    #[test]
    fn nth_value_preserves_index_then_selects_one_value_or_nil() {
        let span = Span::new(0, 1);
        for (index, producer, expected) in [
            (
                1,
                Value::values(vec![Value::Integer(4), Value::Integer(5)]),
                Value::Integer(5),
            ),
            (0, Value::Integer(4), Value::Integer(4)),
            (0, Value::values(Vec::new()), Value::Nil),
            (2, Value::Integer(4), Value::Nil),
        ] {
            let mut stack = vec![Value::Integer(99), Value::Integer(index)];
            execute_nth_value(&Instruction::CheckNthValueIndex(span), &mut stack, span)
                .unwrap_or_else(|error| panic!("valid index: {error}"));
            assert!(matches!(
                stack.as_slice(),
                [Value::Integer(99), Value::Integer(actual)] if *actual == index
            ));
            stack.push(producer);
            execute_nth_value(&Instruction::NthValue, &mut stack, span)
                .unwrap_or_else(|error| panic!("valid selection: {error}"));
            let [Value::Integer(99), actual] = stack.as_slice() else {
                panic!("selection must preserve the surrounding stack: {stack:?}");
            };
            match (actual, expected) {
                (Value::Integer(actual), Value::Integer(expected)) => {
                    assert_eq!(*actual, expected);
                }
                (Value::Nil, Value::Nil) => {}
                (actual, expected) => panic!("expected {expected:?}, got {actual:?}"),
            }
        }
    }
}

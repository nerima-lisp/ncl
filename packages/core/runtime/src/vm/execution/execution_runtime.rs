#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    mut environment: Environment,
    span: Span,
    start_program_counter: usize,
) -> Result<Value, RuntimeError> {
    let mut stack = Vec::with_capacity(function.instructions.len());
    let mut scopes: Vec<(Environment, usize, usize)> = Vec::new();
    let _dynamic_guard = runtime.dynamic_guard();
    let mut program_counter = start_program_counter;

    loop {
        let Some(instruction) = function.instructions.get(program_counter) else {
            return Err(invalid(
                "compiled function reached an invalid instruction pointer",
                span,
            ));
        };

        let mut pre_control_context = PreControlInstructionContext {
            runtime,
            program,
            function,
            stack: &mut stack,
            scopes: &mut scopes,
            environment: &mut environment,
            program_counter: &mut program_counter,
            span,
        };
        if execute_pre_control_instruction(instruction, &mut pre_control_context)? {
            continue;
        }

        let mut control_context = ControlInstructionContext {
            runtime,
            program,
            stack: &mut stack,
            environment: &environment,
            program_counter: &mut program_counter,
            span,
        };
        if execute_control_instruction(instruction, &mut control_context)? {
            continue;
        }

        if execute_value_instruction(
            runtime,
            instruction,
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        )? {
            continue;
        }

        if let Some(value) =
            execute_return_instruction(instruction, &mut stack, &environment, &scopes, span)?
        {
            return Ok(value);
        }

        unreachable!("load instruction was not handled before dispatch");
    }
}

fn execute_return_instruction(
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    scopes: &[(Environment, usize, usize)],
    span: Span,
) -> Result<Option<Value>, RuntimeError> {
    match instruction {
        Instruction::ReturnFrom { name } => {
            let value = pop_value(stack, span, "return-from")?;
            Err(RuntimeError::ReturnFrom {
                block: name.clone(),
                target: environment.lookup_block(name),
                value: ReturnValue::new(value),
                span: Some(span),
            })
        }
        Instruction::Return => {
            if !scopes.is_empty() {
                return Err(invalid(
                    "compiled function returned with an open scope",
                    span,
                ));
            }
            Ok(Some(pop_value(stack, span, "return")?))
        }
        _ => Ok(None),
    }
}

fn execute_value_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Eval(form_span) => {
            let value = pop_value(stack, span, "eval")?.primary_value();
            let form = Runtime::form_from_value(&value, *form_span)?;
            stack.push(runtime.eval_values_in(&form, environment)?);
        }
        Instruction::Call(argument_count) => {
            execute_call_instruction(runtime, *argument_count, stack, environment, span)?;
        }
        Instruction::Apply(argument_count) => {
            execute_apply_instruction(runtime, *argument_count, stack, environment, span)?;
        }
        Instruction::MapCar(sequence_count) => {
            execute_mapcar_instruction(runtime, *sequence_count, stack, environment, span)?;
        }
        Instruction::MultipleValueCall(value_form_count) => {
            execute_multiple_value_call_instruction(
                runtime,
                *value_form_count,
                stack,
                environment,
                span,
            )?;
        }
        _ => return Ok(false),
    }
    *program_counter += 1;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(instructions: Vec<Instruction>) -> FunctionCode {
        FunctionCode {
            name: Some("test-function".to_string()),
            parameters: Vec::new(),
            required_escaped: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            rest_escaped: false,
            auxiliary: Vec::new(),
            instructions,
        }
    }

    #[test]
    fn rejects_invalid_instruction_pointer() {
        let runtime = Runtime::new();
        let program = Rc::new(Program {
            functions: vec![function(Vec::new())],
            entry: 0,
        });
        let error = match run_code_from(
            &runtime,
            &program,
            &program.functions[0],
            Environment::new(),
            Span::new(0, 1),
            0,
        ) {
            Err(error) => error,
            Ok(value) => panic!("unexpected successful result: {value:?}"),
        };
        assert!(
            matches!(error, RuntimeError::InvalidForm { message, .. } if message == "compiled function reached an invalid instruction pointer")
        );
    }

    #[test]
    fn rejects_return_with_an_open_scope() {
        let span = Span::new(0, 1);
        let error = match execute_return_instruction(
            &Instruction::Return,
            &mut vec![Value::Nil],
            &Environment::new(),
            &[(Environment::new(), 0, 0)],
            span,
        ) {
            Err(error) => error,
            Ok(value) => panic!("unexpected successful result: {value:?}"),
        };
        assert!(
            matches!(error, RuntimeError::InvalidForm { message, .. } if message == "compiled function returned with an open scope")
        );
    }

    #[test]
    fn returns_the_top_value_when_the_scope_is_closed() {
        let span = Span::new(0, 1);
        let result = execute_return_instruction(
            &Instruction::Return,
            &mut vec![Value::Integer(42)],
            &Environment::new(),
            &[],
            span,
        );

        assert!(matches!(result, Ok(Some(Value::Integer(42)))));
    }

    #[test]
    fn return_instructions_reject_missing_values() {
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases = [
            (Instruction::Return, "return has no value on the stack"),
            (
                Instruction::ReturnFrom {
                    name: "DONE".to_string(),
                },
                "return-from has no value on the stack",
            ),
        ];

        for (instruction, expected) in cases {
            let result =
                execute_return_instruction(&instruction, &mut Vec::new(), &environment, &[], span)
                    .map(|_| ());
            assert!(
                matches!(result, Err(RuntimeError::InvalidForm { message, .. }) if message == expected)
            );
        }
    }

    #[test]
    fn non_return_instructions_leave_return_processing_unchanged() {
        let result = execute_return_instruction(
            &Instruction::Pop,
            &mut vec![Value::Integer(7)],
            &Environment::new(),
            &[],
            Span::new(0, 1),
        );

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn return_from_preserves_the_block_target_and_value() {
        let span = Span::new(0, 1);
        let environment = Environment::new();
        let result = execute_return_instruction(
            &Instruction::ReturnFrom {
                name: "DONE".to_string(),
            },
            &mut vec![Value::Integer(7)],
            &environment,
            &[],
            span,
        );

        assert!(matches!(
            result,
            Err(RuntimeError::ReturnFrom {
                block,
                target: None,
                value,
                ..
            }) if block == "DONE" && value == ReturnValue::new(Value::Integer(7))
        ));
    }
}

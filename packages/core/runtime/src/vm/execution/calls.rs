fn execute_call_instruction(
    instruction: &Instruction,
    state: &mut ExecutionState<'_>,
) -> Result<Option<ExecutionOutcome>, RuntimeError> {
    let runtime = state.runtime;
    let environment = &mut *state.environment;
    let stack = &mut *state.stack;
    let scopes = &mut *state.scopes;
    let span = state.span;
    let program_counter = &mut *state.program_counter;

    match instruction {
            Instruction::Eval(form_span) => {
                let value = pop_value(stack, span, "eval")?.primary_value();
                let form = runtime.form_from_value(&value, *form_span)?;
                stack.push(runtime.eval_values_in(&form, environment)?);
                *program_counter += 1;
            }
            Instruction::Call(argument_count) => {
                if stack.len() < argument_count.saturating_add(1) {
                    return Err(invalid("call has too few stack values", span));
                }
                let arguments_start = stack.len() - argument_count;
                let arguments = stack.split_off(arguments_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("call has no function value", span))?;
                let arguments = arguments
                    .into_iter()
                    .map(|value| value.primary_value())
                    .collect::<Vec<_>>();
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    environment,
                )?);
                *program_counter += 1;
            }
            Instruction::Apply(argument_count) => {
                if *argument_count == 0 || stack.len() < argument_count.saturating_add(1) {
                    return Err(invalid("apply has too few stack values", span));
                }
                let arguments_start = stack.len() - argument_count;
                let mut evaluated = stack.split_off(arguments_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("apply has no function value", span))?;
                let final_value = evaluated
                    .pop()
                    .ok_or_else(|| invalid("apply has no final list", span))?;
                let mut arguments = evaluated
                    .into_iter()
                    .map(|value| value.primary_value())
                    .collect::<Vec<_>>();
                let mut final_arguments = final_value
                    .primary_value()
                    .list_items()
                    .ok_or_else(|| invalid("apply's final argument must be a proper list", span))?;
                arguments.append(&mut final_arguments);
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    environment,
                )?);
                *program_counter += 1;
            }
            Instruction::MapCar(sequence_count) => {
                if *sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
                    return Err(invalid("mapcar has too few stack values", span));
                }
                let sequences_start = stack.len() - *sequence_count;
                let sequences = stack.split_off(sequences_start);
                let function_value = stack
                    .pop()
                    .ok_or_else(|| invalid("mapcar has no function value", span))?;
                let lists = sequences
                    .iter()
                    .map(|value| {
                        value
                            .primary_value()
                            .list_items()
                            .ok_or_else(|| invalid("mapcar arguments must be proper lists", span))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let length = lists.iter().map(Vec::len).min().unwrap_or(0);
                let mut results = Vec::with_capacity(length);
                for index in 0..length {
                    let arguments = lists
                        .iter()
                        .map(|items| items[index].clone())
                        .collect::<Vec<_>>();
                    results.push(
                        runtime
                            .apply_in(
                                &function_value.primary_value(),
                                &arguments,
                                span,
                                environment,
                            )?
                            .primary_value(),
                    );
                }
                stack.push(Value::list(results));
                *program_counter += 1;
            }
            Instruction::MultipleValueCall(value_form_count) => {
                if stack.len() < value_form_count.saturating_add(1) {
                    return Err(invalid(
                        "multiple-value-call has too few stack values",
                        span,
                    ));
                }
                let start = stack.len() - value_form_count.saturating_add(1);
                let mut operands = stack.split_off(start);
                let function_value = operands
                    .first()
                    .cloned()
                    .ok_or_else(|| invalid("multiple-value-call has no function value", span))?;
                let mut arguments = Vec::new();
                for value in operands.drain(1..) {
                    arguments.extend(value.multiple_values());
                }
                stack.push(runtime.apply_in(
                    &function_value.primary_value(),
                    &arguments,
                    span,
                    environment,
                )?);
                *program_counter += 1;
            }
            Instruction::Return => {
                if !scopes.is_empty() {
                    return Err(invalid(
                        "compiled function returned with an open scope",
                        span,
                    ));
                }
                return Ok(Some(ExecutionOutcome::Return(
                    pop_value(stack, span, "return")?,
                )));
            }
            _ => return Ok(None),
    }

    Ok(Some(ExecutionOutcome::Continue))
}

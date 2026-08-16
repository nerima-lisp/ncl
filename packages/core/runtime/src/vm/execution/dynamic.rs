fn execute_dynamic_instruction(
    instruction: &Instruction,
    state: &mut ExecutionState<'_>,
) -> Result<Option<ExecutionOutcome>, RuntimeError> {
    let runtime = state.runtime;
    let program = state.program;
    let environment = &mut *state.environment;
    let stack = &mut *state.stack;
    let span = state.span;
    let program_counter = &mut *state.program_counter;

    match instruction {
            Instruction::Progv {
                symbols,
                values,
                body,
            } => {
                let symbols_function = program.functions.get(*symbols).ok_or_else(|| {
                    invalid("compiled progv symbol function id is out of range", span)
                })?;
                let symbols_value = run_code(
                    runtime,
                    program,
                    symbols_function,
                    environment.clone(),
                    span,
                )?
                .primary_value();
                let symbol_items =
                    symbols_value
                        .list_items()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "LIST".to_string(),
                            actual: symbols_value.type_name().to_string(),
                            span: Some(span),
                        })?;

                let values_function = program.functions.get(*values).ok_or_else(|| {
                    invalid("compiled progv value function id is out of range", span)
                })?;
                let values_value =
                    run_code(runtime, program, values_function, environment.clone(), span)?
                        .primary_value();
                let value_items = values_value
                    .list_items()
                    .ok_or_else(|| RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: values_value.type_name().to_string(),
                        span: Some(span),
                    })?;

                let _dynamic_guard = runtime.dynamic_guard();
                for (index, symbol) in symbol_items.iter().enumerate() {
                    let name = symbol.symbol_name().ok_or_else(|| {
                        invalid("progv symbol list must contain only symbols", span)
                    })?;
                    runtime.define_dynamic(
                        name,
                        value_items.get(index).cloned().unwrap_or(Value::Nil),
                    );
                }
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled progv body function id is out of range", span)
                })?;
                stack.push(run_code(
                    runtime,
                    program,
                    body_function,
                    environment.clone(),
                    span,
                )?);
                *program_counter += 1;
            }
            Instruction::Throw => {
                let value = pop_value(stack, span, "throw")?;
                let tag = pop_value(stack, span, "throw")?.primary_value();
                return Err(RuntimeError::Throw {
                    tag: ThrowTag::new(tag),
                    value: ReturnValue::new(value),
                    span: Some(span),
                });
            }
            Instruction::Block {
                function: function_id,
                name,
            } => {
                let function = program
                    .functions
                    .get(*function_id)
                    .ok_or_else(|| invalid("compiled block function id is out of range", span))?;
                let target = runtime.fresh_block_target();
                let block_environment = environment.child();
                block_environment.define_block(name, target);
                match run_code(runtime, program, function, block_environment, span) {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::ReturnFrom {
                        target: Some(return_target),
                        value,
                        ..
                    }) if return_target == target => {
                        stack.push(value.into_value());
                    }
                    Err(error) => return Err(error),
                }
                *program_counter += 1;
            }
            Instruction::TagBody {
                function: function_id,
                tags,
            } => {
                let tagbody_function = program
                    .functions
                    .get(*function_id)
                    .ok_or_else(|| invalid("compiled tagbody function id is out of range", span))?;
                let target = runtime.fresh_block_target();
                let tag_environment = environment.child();
                for (tag, _) in tags {
                    tag_environment.define_tag(tag, target);
                }

                let mut tagbody_program_counter = 0;
                loop {
                    match run_code_from(
                        runtime,
                        program,
                        tagbody_function,
                        tag_environment.clone(),
                        span,
                        tagbody_program_counter,
                    ) {
                        Ok(_) => {
                            stack.push(Value::Nil);
                            break;
                        }
                        Err(RuntimeError::Go {
                            tag,
                            target: Some(go_target),
                            ..
                        }) if go_target == target => {
                            tagbody_program_counter = tags
                                .iter()
                                .find(|(known_tag, _)| known_tag == &tag)
                                .map(|(_, position)| *position)
                                .ok_or_else(|| {
                                    invalid("compiled GO target is missing from TAGBODY", span)
                                })?;
                        }
                        Err(error) => return Err(error),
                    }
                }
                *program_counter += 1;
            }
            Instruction::Go { tag } => {
                return Err(RuntimeError::Go {
                    tag: tag.clone(),
                    target: environment.lookup_tag(tag),
                    span: Some(span),
                });
            }
            Instruction::UnwindProtect {
                protected: protected_id,
                cleanup: cleanup_id,
            } => {
                let protected_function = program.functions.get(*protected_id).ok_or_else(|| {
                    invalid(
                        "compiled unwind-protect protected function id is out of range",
                        span,
                    )
                })?;
                let cleanup_function = program.functions.get(*cleanup_id).ok_or_else(|| {
                    invalid(
                        "compiled unwind-protect cleanup function id is out of range",
                        span,
                    )
                })?;
                let protected_result = run_code(
                    runtime,
                    program,
                    protected_function,
                    environment.clone(),
                    span,
                );
                let cleanup_result = run_code(
                    runtime,
                    program,
                    cleanup_function,
                    environment.clone(),
                    span,
                );
                match cleanup_result {
                    Ok(_) => stack.push(protected_result?),
                    Err(error) => return Err(error),
                }
                *program_counter += 1;
            }
            Instruction::ReturnFrom { name } => {
                let value = pop_value(stack, span, "return-from")?;
                return Err(RuntimeError::ReturnFrom {
                    block: name.clone(),
                    target: environment.lookup_block(name),
                    value: ReturnValue::new(value),
                    span: Some(span),
                });
            }
            _ => return Ok(None),
    }

    Ok(Some(ExecutionOutcome::Continue))
}

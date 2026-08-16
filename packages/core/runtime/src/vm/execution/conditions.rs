fn execute_condition_instruction(
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
            Instruction::IgnoreErrors(function_id) => {
                let function = program.functions.get(*function_id).ok_or_else(|| {
                    invalid("compiled ignore-errors function id is out of range", span)
                })?;
                match run_code(runtime, program, function, environment.clone(), span) {
                    Ok(value) => stack.push(value),
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error) => {
                        stack.push(Value::values(vec![Value::Nil, Value::condition(&error)]));
                    }
                }
                *program_counter += 1;
            }
            Instruction::HandlerCase { protected, clauses } => {
                let protected_function = program.functions.get(*protected).ok_or_else(|| {
                    invalid("compiled handler-case function id is out of range", span)
                })?;
                let guard = runtime.condition_handler_guard(
                    clauses
                        .iter()
                        .map(|clause| ConditionHandlerBinding {
                            condition: clause.condition.clone(),
                            function: None,
                            catch: true,
                        })
                        .collect(),
                );
                let protected_result = run_code(
                    runtime,
                    program,
                    protected_function,
                    environment.clone(),
                    span,
                );
                drop(guard);
                match protected_result {
                    Ok(value) => stack.push(value),
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error) => {
                        let Some(clause) = clauses
                            .iter()
                            .find(|clause| error.matches_condition(&clause.condition))
                        else {
                            return Err(error);
                        };
                        program.functions.get(clause.function).ok_or_else(|| {
                            invalid("compiled handler-case clause id is out of range", span)
                        })?;
                        let arguments = if clause.variable.is_some() {
                            vec![Value::condition(&error)]
                        } else {
                            Vec::new()
                        };
                        stack.push(run(
                            runtime,
                            program.clone(),
                            clause.function,
                            environment.clone(),
                            &arguments,
                            span,
                        )?);
                    }
                }
                *program_counter += 1;
            }
            Instruction::HandlerBind { body, handlers } => {
                let handler_bindings = handlers
                    .iter()
                    .map(|handler| {
                        program.functions.get(handler.function).ok_or_else(|| {
                            invalid("compiled handler-bind clause id is out of range", span)
                        })?;
                        Ok(ConditionHandlerBinding {
                            condition: handler.condition.clone(),
                            function: Some(Value::compiled(
                                program.clone(),
                                handler.function,
                                environment.clone(),
                            )),
                            catch: false,
                        })
                    })
                    .collect::<Result<Vec<_>, RuntimeError>>()?;
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled handler-bind body id is out of range", span)
                })?;
                let guard = runtime.condition_handler_guard(handler_bindings);
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
                    Ok(value) => stack.push(value),
                    Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
                    Err(error @ RuntimeError::Go { .. }) => return Err(error),
                    Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
                    Err(error @ RuntimeError::Signaled { .. }) => return Err(error),
                    Err(error) => {
                        let Some(handler) = handlers
                            .iter()
                            .find(|handler| error.matches_condition(&handler.condition))
                        else {
                            return Err(error);
                        };
                        program.functions.get(handler.function).ok_or_else(|| {
                            invalid("compiled handler-bind clause id is out of range", span)
                        })?;
                        stack.push(run(
                            runtime,
                            program.clone(),
                            handler.function,
                            environment.clone(),
                            &[Value::condition(&error)],
                            span,
                        )?);
                    }
                }
                *program_counter += 1;
            }
            Instruction::RestartBind { body, bindings } => {
                let mut restarts = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let binding_function =
                        program.functions.get(binding.function).ok_or_else(|| {
                            invalid("compiled restart-bind clause id is out of range", span)
                        })?;
                    let function = run_code(
                        runtime,
                        program,
                        binding_function,
                        environment.clone(),
                        span,
                    )?
                    .primary_value();
                    restarts.push((binding.name.as_str(), function));
                }
                let guard = runtime.restart_guard(
                    restarts
                        .iter()
                        .map(|(name, function)| {
                            RestartBinding::new((*name).to_string(), Some(function.clone()))
                        })
                        .collect(),
                );
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled restart-bind body id is out of range", span)
                })?;
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
                    Ok(value) => stack.push(value),
                    Err(error) => {
                        let RuntimeError::InvokeRestart {
                            name: invoked,
                            arguments,
                            ..
                        } = &error
                        else {
                            return Err(error);
                        };
                        let Some((_, function)) = restarts
                            .iter()
                            .find(|(name, _)| normalize_name(invoked.as_str()) == *name)
                        else {
                            return Err(error);
                        };
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        stack.push(runtime.apply_in(
                            function,
                            &argument_values,
                            span,
                            environment,
                        )?);
                    }
                }
                *program_counter += 1;
            }
            Instruction::Catch { tag, body } => {
                let tag_function = program.functions.get(*tag).ok_or_else(|| {
                    invalid("compiled catch tag function id is out of range", span)
                })?;
                let tag = run_code(runtime, program, tag_function, environment.clone(), span)?
                    .primary_value();
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled catch body function id is out of range", span)
                })?;
                match run_code(runtime, program, body_function, environment.clone(), span) {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::Throw {
                        tag: thrown_tag,
                        value,
                        ..
                    }) if thrown_tag.matches(&tag) => stack.push(value.into_value()),
                    Err(error) => return Err(error),
                }
                *program_counter += 1;
            }
            Instruction::WithSimpleRestart { name, body } => {
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled with-simple-restart body id is out of range", span)
                })?;
                let guard = runtime.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                match body_result {
                    Ok(value) => stack.push(value),
                    Err(RuntimeError::InvokeRestart {
                        name: invoked,
                        value,
                        ..
                    }) if normalize_name(invoked.as_str()) == *name => {
                        stack.push(value.into_value());
                    }
                    Err(error) => return Err(error),
                }
                *program_counter += 1;
            }
            Instruction::RestartCase { protected, clauses } => {
                let protected_function = program.functions.get(*protected).ok_or_else(|| {
                    invalid(
                        "compiled restart-case protected function id is out of range",
                        span,
                    )
                })?;
                let guard = runtime.restart_guard(
                    clauses
                        .iter()
                        .map(|clause| RestartBinding::new(clause.name.clone(), None))
                        .collect(),
                );
                let protected_result = run_code(
                    runtime,
                    program,
                    protected_function,
                    environment.clone(),
                    span,
                );
                drop(guard);
                match protected_result {
                    Ok(value) => stack.push(value),
                    Err(error) => {
                        let RuntimeError::InvokeRestart {
                            name: invoked,
                            arguments,
                            ..
                        } = &error
                        else {
                            return Err(error);
                        };
                        let Some(clause) = clauses.iter().find(|clause| {
                            normalize_name(invoked.as_str()) == clause.name.as_str()
                        }) else {
                            return Err(error);
                        };
                        program.functions.get(clause.function).ok_or_else(|| {
                            invalid("compiled restart-case clause id is out of range", span)
                        })?;
                        let argument_values = arguments
                            .iter()
                            .cloned()
                            .map(ReturnValue::into_value)
                            .collect::<Vec<_>>();
                        stack.push(run(
                            runtime,
                            program.clone(),
                            clause.function,
                            environment.clone(),
                            &argument_values,
                            span,
                        )?);
                    }
                }
                *program_counter += 1;
            }
            Instruction::WithConditionRestarts {
                condition,
                restarts,
                body,
            } => {
                let condition_function = program.functions.get(*condition).ok_or_else(|| {
                    invalid(
                        "compiled with-condition-restarts condition function id is out of range",
                        span,
                    )
                })?;
                let condition_value = run_code(
                    runtime,
                    program,
                    condition_function,
                    environment.clone(),
                    span,
                )?
                .primary_value();
                if condition_value.condition_type_name().is_none() {
                    return Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition_value.type_name().to_string(),
                        span: Some(span),
                    });
                }

                let restarts_function = program.functions.get(*restarts).ok_or_else(|| {
                    invalid(
                        "compiled with-condition-restarts restarts function id is out of range",
                        span,
                    )
                })?;
                let restarts_value = run_code(
                    runtime,
                    program,
                    restarts_function,
                    environment.clone(),
                    span,
                )?
                .primary_value();
                let Some(restart_values) = restarts_value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: restarts_value.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if let Some(restart) = restart_values
                    .iter()
                    .find(|restart| restart.restart_name().is_none())
                {
                    return Err(RuntimeError::Type {
                        expected: "RESTART".to_string(),
                        actual: restart.type_name().to_string(),
                        span: Some(span),
                    });
                }

                let guard = runtime.condition_restart_guard(condition_value, restart_values);
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid(
                        "compiled with-condition-restarts body id is out of range",
                        span,
                    )
                })?;
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                stack.push(body_result?);
                *program_counter += 1;
            }
            _ => return Ok(None),
    }

    Ok(Some(ExecutionOutcome::Continue))
}

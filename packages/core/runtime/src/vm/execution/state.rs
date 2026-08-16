fn execute_state_instruction(
    instruction: &Instruction,
    state: &mut ExecutionState<'_>,
) -> Result<Option<ExecutionOutcome>, RuntimeError> {
    let runtime = state.runtime;
    let program = state.program;
    let function = state.function;
    let environment = &mut *state.environment;
    let stack = &mut *state.stack;
    let scopes = &mut *state.scopes;
    let span = state.span;
    let program_counter = &mut *state.program_counter;

    match instruction {
            Instruction::Constant(constant) => {
                stack.push(constant_value(constant));
                *program_counter += 1;
            }
            Instruction::Quote(form) => {
                stack.push(runtime.quoted_value(form)?);
                *program_counter += 1;
            }
            Instruction::QuasiQuote(form) => {
                stack.push(runtime.quasiquote_value(form, environment)?);
                *program_counter += 1;
            }
            Instruction::Load(name) => {
                let value = runtime.lookup_in(name, environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                *program_counter += 1;
            }
            Instruction::LoadExact(name) => {
                let value = runtime.lookup_exact_in(name, environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                *program_counter += 1;
            }
            Instruction::FunctionLoad(name) => {
                let value = runtime
                    .lookup_function_in(name, environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                *program_counter += 1;
            }
            Instruction::FunctionLoadExact(name) => {
                let value = runtime
                    .lookup_function_exact_in(name, environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                *program_counter += 1;
            }
            Instruction::IsBound(name) => {
                stack.push(Value::boolean(runtime.is_bound_in(name, environment)));
                *program_counter += 1;
            }
            Instruction::IsBoundExact(name) => {
                stack.push(Value::boolean(
                    runtime.lookup_exact_in(name, environment).is_some(),
                ));
                *program_counter += 1;
            }
            Instruction::Define(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_in(name, value.clone(), environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::DefineExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_exact_in(name, value.clone(), environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::DefineFunction(name) => {
                let value = pop_value(stack, span, "local function definition")?;
                environment.define_function(name, value);
                *program_counter += 1;
            }
            Instruction::DefineFunctionExact(name) => {
                let value = pop_value(stack, span, "local function definition")?;
                environment.define_function_exact(name, value);
                *program_counter += 1;
            }
            Instruction::DefineFunctionDocumentation {
                name,
                exact,
                documentation,
            } => {
                if *exact {
                    environment.define_function_documentation_exact(name, documentation);
                } else {
                    environment.define_function_documentation(name, documentation);
                }
                *program_counter += 1;
            }
            Instruction::DefineVariableDocumentation {
                name,
                exact,
                documentation,
            } => {
                if *exact {
                    environment.define_variable_documentation_exact(name, documentation);
                } else {
                    environment.define_variable_documentation(name, documentation);
                }
                *program_counter += 1;
            }
            Instruction::DefineSpecial { name, force } => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
                if *force && runtime.is_constant_in(name) {
                    return Err(runtime.constant_modification_error(name, span));
                }
                let value = runtime.define_special_value(name, value.primary_value(), *force);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))? =
                    value;
                *program_counter += 1;
            }
            Instruction::DefineSpecialExact { name, force } => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
                if *force && runtime.is_constant_exact_in(name) {
                    return Err(runtime.constant_modification_error(name, span));
                }
                let value = runtime.define_special_value_exact(name, value.primary_value(), *force);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))? =
                    value;
                *program_counter += 1;
            }
            Instruction::DefineValues(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_in(name, value, environment);
                *program_counter += 1;
            }
            Instruction::DefineValuesExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_exact_in(name, value, environment);
                *program_counter += 1;
            }
            Instruction::Set(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::SetExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::Setf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))?;
                let value = if setf_place_uses_multiple_values(place) {
                    value
                } else {
                    value.primary_value()
                };
                runtime.set_place(place, value.clone(), environment)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::MapIntoSetf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("map-into has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_map_into_destination(place, value.clone(), environment)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("map-into has no value on the stack", span))? = value;
                *program_counter += 1;
            }
            Instruction::Psetq(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for (name, value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
                stack.push(Value::Nil);
                *program_counter += 1;
            }
            Instruction::PsetqExact(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for ((name, escaped), value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, environment, span)?;
                    }
                }
                stack.push(Value::Nil);
                *program_counter += 1;
            }
            Instruction::MultipleValueSetq(names) => {
                let source = pop_value(stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
                stack.push(source.primary_value());
                *program_counter += 1;
            }
            Instruction::MultipleValueSetqExact(names) => {
                let source = pop_value(stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, environment, span)?;
                    }
                }
                stack.push(source.primary_value());
                *program_counter += 1;
            }
            Instruction::EnterScope => {
                scopes.push((
                    environment.clone(),
                    runtime.dynamic_depth(),
                    runtime.exact_dynamic_depth(),
                ));
                *environment = environment.child();
                *program_counter += 1;
            }
            Instruction::ExitScope => {
                let (parent, depth, exact_depth) = scopes
                    .pop()
                    .ok_or_else(|| invalid("scope exit has no matching scope", span))?;
                runtime.truncate_dynamic(depth);
                runtime.truncate_exact_dynamic(exact_depth);
                *environment = parent;
                *program_counter += 1;
            }
            Instruction::Pop => {
                pop_value(stack, span, "pop")?;
                *program_counter += 1;
            }
            Instruction::Dup => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("dup has no value on the stack", span))?;
                stack.push(value);
                *program_counter += 1;
            }
            Instruction::Primary => {
                let value = pop_value(stack, span, "primary value")?;
                stack.push(value.primary_value());
                *program_counter += 1;
            }
            Instruction::Values(value_count) => {
                if stack.len() < *value_count {
                    return Err(invalid("values has too few stack values", span));
                }
                let values = stack.split_off(stack.len() - *value_count);
                stack.push(Value::values(values));
                *program_counter += 1;
            }
            Instruction::NthValue(index_span) => {
                let values = pop_value(stack, span, "nth-value values")?;
                let index = pop_value(stack, span, "nth-value index")?.primary_value();
                let index = match index {
                    Value::Integer(index) if index >= 0 => {
                        usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
                    }
                    Value::Integer(_) => {
                        return Err(RuntimeError::InvalidForm {
                            message: "nth-value index must be non-negative".to_string(),
                            span: Some(*index_span),
                        });
                    }
                    value => {
                        return Err(RuntimeError::Type {
                            expected: "INTEGER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(*index_span),
                        });
                    }
                };
                stack.push(
                    values
                        .multiple_values()
                        .get(index)
                        .cloned()
                        .unwrap_or(Value::Nil),
                );
                *program_counter += 1;
            }
            Instruction::MultipleValueList => {
                let value = pop_value(stack, span, "multiple-value-list")?;
                stack.push(Value::list(value.multiple_values()));
                *program_counter += 1;
            }
            Instruction::BindValues(names) => {
                let value = pop_value(stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    runtime.define_in(
                        name,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        environment,
                    );
                }
                *program_counter += 1;
            }
            Instruction::BindValuesExact(names) => {
                let value = pop_value(stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.define_exact_in(name, value, environment);
                    } else {
                        runtime.define_in(name, value, environment);
                    }
                }
                *program_counter += 1;
            }
            Instruction::Destructure(specification) => {
                let value = pop_value(stack, span, "destructuring-bind")?;
                destructure_specification(
                    specification,
                    value.primary_value(),
                    runtime,
                    program,
                    environment,
                    span,
                )?;
                *program_counter += 1;
            }
            Instruction::JumpIfFalse(target) => {
                let condition = pop_value(stack, span, "conditional jump")?;
                if condition.is_truthy() {
                    *program_counter += 1;
                } else {
                    *program_counter = jump_target(function, *target, span)?;
                }
            }
            Instruction::Jump(target) => {
                *program_counter = jump_target(function, *target, span)?;
            }
            Instruction::MakeClosure(function_id) => {
                if *function_id >= program.functions.len() {
                    return Err(invalid("compiled closure id is out of range", span));
                }
                stack.push(Value::compiled(
                    program.clone(),
                    *function_id,
                    environment.clone(),
                ));
                *program_counter += 1;
            }
            _ => return Ok(None),
    }

    Ok(Some(ExecutionOutcome::Continue))
}

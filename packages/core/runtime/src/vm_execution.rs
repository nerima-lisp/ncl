#![allow(clippy::wildcard_imports)]
use super::*;

#[path = "vm_execution_runtime.rs"]
mod execution_runtime;

pub(super) fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
    start: usize,
) -> Result<Value, RuntimeError> {
    execution_runtime::run_code_from(runtime, program, function, environment, span, start)
}

pub(super) fn execute_call_instruction(
    runtime: &Runtime,
    argument_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
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
    Ok(())
}

pub(super) fn execute_apply_instruction(
    runtime: &Runtime,
    argument_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if argument_count == 0 || stack.len() < argument_count.saturating_add(1) {
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
    Ok(())
}

pub(super) fn execute_mapcar_instruction(
    runtime: &Runtime,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
        return Err(invalid("mapcar has too few stack values", span));
    }
    let sequences_start = stack.len() - sequence_count;
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
    Ok(())
}

pub(super) fn execute_multiple_value_call_instruction(
    runtime: &Runtime,
    value_form_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
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
    Ok(())
}

pub(super) fn execute_handler_case_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    protected: usize,
    clauses: &[HandlerCaseClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let protected_function = program
        .functions
        .get(protected)
        .ok_or_else(|| invalid("compiled handler-case function id is out of range", span))?;
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
        Err(
            error @ (RuntimeError::ReturnFrom { .. }
            | RuntimeError::Go { .. }
            | RuntimeError::InvokeRestart { .. }),
        ) => return Err(error),
        Err(error) => {
            let Some(clause) = clauses
                .iter()
                .find(|clause| error.matches_condition(&clause.condition))
            else {
                return Err(error);
            };
            program
                .functions
                .get(clause.function)
                .ok_or_else(|| invalid("compiled handler-case clause id is out of range", span))?;
            let arguments = if clause.variable.is_some() {
                vec![Value::condition(&error)]
            } else {
                Vec::new()
            };
            stack.push(run(
                runtime,
                program,
                clause.function,
                environment,
                &arguments,
                span,
            )?);
        }
    }
    Ok(())
}

pub(super) fn execute_handler_bind_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    body: usize,
    handlers: &[HandlerBindClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let handler_bindings = handlers
        .iter()
        .map(|handler| {
            program
                .functions
                .get(handler.function)
                .ok_or_else(|| invalid("compiled handler-bind clause id is out of range", span))?;
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
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled handler-bind body id is out of range", span))?;
    let guard = runtime.condition_handler_guard(handler_bindings);
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    match body_result {
        Ok(value) => stack.push(value),
        Err(
            error @ (RuntimeError::ReturnFrom { .. }
            | RuntimeError::Go { .. }
            | RuntimeError::InvokeRestart { .. }
            | RuntimeError::Signaled(_)),
        ) => return Err(error),
        Err(error) => {
            let Some(handler) = handlers
                .iter()
                .find(|handler| error.matches_condition(&handler.condition))
            else {
                return Err(error);
            };
            program
                .functions
                .get(handler.function)
                .ok_or_else(|| invalid("compiled handler-bind clause id is out of range", span))?;
            stack.push(run(
                runtime,
                program,
                handler.function,
                environment,
                &[Value::condition(&error)],
                span,
            )?);
        }
    }
    Ok(())
}

pub(super) fn execute_load_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value =
        match instruction {
            Instruction::Constant(constant) => constant_value(constant, span)?,
            Instruction::Quote(form) => Runtime::quoted_value(form)?,
            Instruction::QuasiQuote(form) => runtime.quasiquote_value(form, environment)?,
            Instruction::Load(name) => runtime.lookup_in(name, environment).ok_or_else(|| {
                RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                }
            })?,
            Instruction::LoadExact(name) => {
                runtime.lookup_exact_in(name, environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?
            }
            Instruction::FunctionLoad(name) => runtime
                .lookup_function_in(name, environment)
                .ok_or_else(|| RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                })?,
            Instruction::FunctionLoadExact(name) => runtime
                .lookup_function_exact_in(name, environment)
                .ok_or_else(|| RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                })?,
            Instruction::IsBound(name) => Value::boolean(runtime.is_bound_in(name, environment)),
            Instruction::IsBoundExact(name) => {
                Value::boolean(runtime.lookup_exact_in(name, environment).is_some())
            }
            _ => return Ok(false),
        };
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_definition_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Define(name) | Instruction::DefineExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Define(_)) {
                runtime.define_in(name, value.clone(), environment);
            } else {
                runtime.define_exact_in(name, value.clone(), environment);
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineFunction(name) | Instruction::DefineFunctionExact(name) => {
            let value = pop_value(stack, span, "local function definition")?;
            if matches!(instruction, Instruction::DefineFunction(_)) {
                environment.define_function(name, value);
            } else {
                environment.define_function_exact(name, value);
            }
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineSpecial { name, force }
        | Instruction::DefineSpecialExact { name, force } => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
            let exact = matches!(instruction, Instruction::DefineSpecialExact { .. });
            if *force
                && if exact {
                    runtime.is_constant_exact_in(name)
                } else {
                    runtime.is_constant_in(name)
                }
            {
                return Err(Runtime::constant_modification_error(name, span));
            }
            let value = if exact {
                runtime.define_special_value_exact(name, value.primary_value(), *force)
            } else {
                runtime.define_special_value(name, value.primary_value(), *force)
            };
            *stack
                .last_mut()
                .ok_or_else(|| invalid("define-special has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineValues(name) | Instruction::DefineValuesExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
            if matches!(instruction, Instruction::DefineValues(_)) {
                runtime.define_in(name, value, environment);
            } else {
                runtime.define_exact_in(name, value, environment);
            }
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn execute_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut [Value],
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Set(name) | Instruction::SetExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("setq has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Set(_)) {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Setf(place) | Instruction::MapIntoSetf(place) => {
            let map_into = matches!(instruction, Instruction::MapIntoSetf(_));
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| {
                    invalid(
                        if map_into {
                            "map-into has no value on the stack"
                        } else {
                            "setf has no value on the stack"
                        },
                        span,
                    )
                })?
                .primary_value();
            if map_into {
                runtime.set_map_into_destination(place, value.clone(), environment)?;
            } else {
                runtime.set_place(place, value.clone(), environment)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn execute_parallel_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
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
            Ok(true)
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
            Ok(true)
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
            Ok(true)
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
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn execute_stack_instruction(
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

pub(super) struct BranchInstructionContext<'a> {
    runtime: &'a Runtime,
    program: &'a Rc<Program>,
    function: &'a FunctionCode,
    stack: &'a mut Vec<Value>,
    environment: &'a Environment,
    program_counter: &'a mut usize,
    span: Span,
}

pub(super) fn execute_binding_and_branch_instruction(
    instruction: &Instruction,
    context: &mut BranchInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    let runtime = context.runtime;
    let program = context.program;
    let function = context.function;
    let stack = &mut *context.stack;
    let environment = context.environment;
    let program_counter = &mut *context.program_counter;
    let span = context.span;
    match instruction {
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
            *program_counter = if condition.is_truthy() {
                *program_counter + 1
            } else {
                jump_target(function, *target, span)?
            };
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
        Instruction::IgnoreErrors(function_id) => {
            let function = program.functions.get(*function_id).ok_or_else(|| {
                invalid("compiled ignore-errors function id is out of range", span)
            })?;
            match run_code(runtime, program, function, environment.clone(), span) {
                Ok(value) => stack.push(value),
                Err(
                    error @ (RuntimeError::ReturnFrom { .. }
                    | RuntimeError::Go { .. }
                    | RuntimeError::InvokeRestart { .. }),
                ) => return Err(error),
                Err(error) => stack.push(Value::values(vec![Value::Nil, Value::condition(&error)])),
            }
            *program_counter += 1;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[path = "vm_control_scopes.rs"]
mod control_scopes;
#[allow(clippy::wildcard_imports)]
use control_scopes::*;

pub(super) struct ControlInstructionContext<'a> {
    runtime: &'a Runtime,
    program: &'a Rc<Program>,
    stack: &'a mut Vec<Value>,
    environment: &'a Environment,
    program_counter: &'a mut usize,
    span: Span,
}

pub(super) fn execute_control_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::HandlerCase { .. }
        | Instruction::HandlerBind { .. }
        | Instruction::RestartBind { .. }
        | Instruction::Catch { .. }
        | Instruction::WithSimpleRestart { .. } => {
            execute_handler_restart_instruction(instruction, context)?;
            Ok(true)
        }
        _ => execute_scope_control_instruction(instruction, context),
    }
}

pub(super) fn execute_scope_control_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::RestartCase { protected, clauses } => execute_restart_case_instruction(
            context.runtime,
            context.program,
            *protected,
            clauses,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::WithConditionRestarts {
            condition,
            restarts,
            body,
        } => execute_with_condition_restarts_instruction(
            context.runtime,
            context.program,
            (*condition, *restarts, *body),
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Progv {
            symbols,
            values,
            body,
        } => execute_progv_instruction(
            context.runtime,
            context.program,
            (*symbols, *values, *body),
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Throw => {
            let value = pop_value(context.stack, context.span, "throw")?;
            let tag = pop_value(context.stack, context.span, "throw")?.primary_value();
            return Err(RuntimeError::Throw {
                tag: ThrowTag::new(tag),
                value: ReturnValue::new(value),
                span: Some(context.span),
            });
        }
        Instruction::Block {
            function: function_id,
            name,
        } => execute_block_instruction(
            context.runtime,
            context.program,
            *function_id,
            name,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::TagBody {
            function: function_id,
            tags,
        } => execute_tagbody_instruction(
            context.runtime,
            context.program,
            *function_id,
            tags,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Go { tag } => {
            return Err(RuntimeError::Go {
                tag: tag.clone(),
                target: context.environment.lookup_tag(tag),
                span: Some(context.span),
            });
        }
        Instruction::UnwindProtect {
            protected: protected_id,
            cleanup: cleanup_id,
        } => execute_unwind_protect_instruction(
            context.runtime,
            context.program,
            (*protected_id, *cleanup_id),
            context.stack,
            context.environment,
            context.span,
        )?,
        _ => return Ok(false),
    }
    *context.program_counter += 1;
    Ok(true)
}

pub(super) fn execute_handler_restart_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<(), RuntimeError> {
    match instruction {
        Instruction::HandlerCase { protected, clauses } => execute_handler_case_instruction(
            context.runtime,
            context.program,
            *protected,
            clauses,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::HandlerBind { body, handlers } => execute_handler_bind_instruction(
            context.runtime,
            context.program,
            *body,
            handlers,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::RestartBind { body, bindings } => execute_restart_bind_instruction(
            context.runtime,
            context.program,
            *body,
            bindings,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Catch { tag, body } => execute_catch_instruction(
            context.runtime,
            context.program,
            *tag,
            *body,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::WithSimpleRestart { name, body } => execute_with_simple_restart_instruction(
            context.runtime,
            context.program,
            name,
            *body,
            context.stack,
            context.environment,
            context.span,
        )?,
        _ => unreachable!("handler/restart instruction was not dispatched"),
    }
    *context.program_counter += 1;
    Ok(())
}

pub(super) struct PreControlInstructionContext<'a> {
    runtime: &'a Runtime,
    program: &'a Rc<Program>,
    function: &'a FunctionCode,
    stack: &'a mut Vec<Value>,
    scopes: &'a mut Vec<(Environment, usize, usize)>,
    environment: &'a mut Environment,
    program_counter: &'a mut usize,
    span: Span,
}

pub(super) fn execute_pre_control_instruction(
    instruction: &Instruction,
    context: &mut PreControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    if execute_load_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_definition_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_set_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_parallel_set_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? {
        return Ok(true);
    }
    if execute_stack_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.scopes,
        context.environment,
        context.program_counter,
        context.span,
    )? {
        return Ok(true);
    }
    let mut branch_context = BranchInstructionContext {
        runtime: context.runtime,
        program: context.program,
        function: context.function,
        stack: context.stack,
        environment: &*context.environment,
        program_counter: context.program_counter,
        span: context.span,
    };
    execute_binding_and_branch_instruction(instruction, &mut branch_context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_program() -> Rc<Program> {
        Rc::new(Program {
            functions: Vec::new(),
            entry: 0,
        })
    }

    fn assert_invalid(result: Result<(), RuntimeError>, expected: &str) {
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. }) if message == expected
        ));
    }

    #[test]
    fn scope_instructions_reject_out_of_range_function_ids() {
        let runtime = Runtime::new();
        let program = empty_program();
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let mut stack = Vec::new();

        assert_invalid(
            execute_restart_bind_instruction(
                &runtime,
                &program,
                0,
                &[],
                &mut stack,
                &environment,
                span,
            ),
            "compiled restart-bind body id is out of range",
        );
        assert_invalid(
            execute_catch_instruction(&runtime, &program, 0, 0, &mut stack, &environment, span),
            "compiled catch tag function id is out of range",
        );
        assert_invalid(
            execute_with_simple_restart_instruction(
                &runtime,
                &program,
                "restart",
                0,
                &mut stack,
                &environment,
                span,
            ),
            "compiled with-simple-restart body id is out of range",
        );
        assert_invalid(
            execute_restart_case_instruction(
                &runtime,
                &program,
                0,
                &[],
                &mut stack,
                &environment,
                span,
            ),
            "compiled restart-case protected function id is out of range",
        );
        assert_invalid(
            execute_with_condition_restarts_instruction(
                &runtime,
                &program,
                (0, 0, 0),
                &mut stack,
                &environment,
                span,
            ),
            "compiled with-condition-restarts condition function id is out of range",
        );
        assert_invalid(
            execute_progv_instruction(
                &runtime,
                &program,
                (0, 0, 0),
                &mut stack,
                &environment,
                span,
            ),
            "compiled progv symbol function id is out of range",
        );
        assert_invalid(
            execute_block_instruction(
                &runtime,
                &program,
                0,
                "block",
                &mut stack,
                &environment,
                span,
            ),
            "compiled block function id is out of range",
        );
        assert_invalid(
            execute_tagbody_instruction(&runtime, &program, 0, &[], &mut stack, &environment, span),
            "compiled tagbody function id is out of range",
        );
        assert_invalid(
            execute_unwind_protect_instruction(
                &runtime,
                &program,
                (0, 0),
                &mut stack,
                &environment,
                span,
            ),
            "compiled unwind-protect protected function id is out of range",
        );
    }

    #[test]
    fn closure_instructions_reject_out_of_range_function_ids() {
        let runtime = Runtime::new();
        let function = FunctionCode {
            name: None,
            parameters: Vec::new(),
            required_escaped: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            rest_escaped: false,
            auxiliary: Vec::new(),
            instructions: vec![Instruction::Return],
        };
        let program = Rc::new(Program {
            functions: vec![function.clone()],
            entry: 0,
        });
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let mut stack = Vec::new();
        let mut program_counter = 0;
        let mut branch_context = BranchInstructionContext {
            runtime: &runtime,
            program: &program,
            function: &function,
            stack: &mut stack,
            environment: &environment,
            program_counter: &mut program_counter,
            span,
        };
        for (instruction, expected) in [
            (
                Instruction::MakeClosure(1),
                "compiled closure id is out of range",
            ),
            (
                Instruction::IgnoreErrors(1),
                "compiled ignore-errors function id is out of range",
            ),
        ] {
            assert_invalid(
                execute_binding_and_branch_instruction(&instruction, &mut branch_context)
                    .map(|_| ()),
                expected,
            );
        }
    }

    #[test]
    fn stack_operations_reject_invalid_shapes() {
        type StackOperation =
            fn(&Runtime, usize, &mut Vec<Value>, &Environment, Span) -> Result<(), RuntimeError>;

        let runtime = Runtime::new();
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases: [(&str, StackOperation, &str); 3] = [
            (
                "call",
                execute_call_instruction,
                "call has too few stack values",
            ),
            (
                "apply",
                execute_apply_instruction,
                "apply has too few stack values",
            ),
            (
                "mapcar",
                execute_mapcar_instruction,
                "mapcar has too few stack values",
            ),
        ];

        for (name, operation, expected) in cases {
            let mut stack = Vec::new();
            assert_invalid(
                operation(&runtime, 0, &mut stack, &environment, span),
                expected,
            );
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn stack_operations_reject_invalid_sequence_shapes() {
        type StackOperation =
            fn(&Runtime, usize, &mut Vec<Value>, &Environment, Span) -> Result<(), RuntimeError>;

        let runtime = Runtime::new();
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases: [(&str, StackOperation, Vec<Value>, &str); 2] = [
            (
                "apply",
                execute_apply_instruction,
                vec![Value::Nil, Value::Integer(1)],
                "apply's final argument must be a proper list",
            ),
            (
                "mapcar",
                execute_mapcar_instruction,
                vec![Value::Nil, Value::Integer(1)],
                "mapcar arguments must be proper lists",
            ),
        ];

        for (name, operation, mut stack, expected) in cases {
            assert_invalid(
                operation(&runtime, 1, &mut stack, &environment, span),
                expected,
            );
            assert!(!name.is_empty());
        }
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

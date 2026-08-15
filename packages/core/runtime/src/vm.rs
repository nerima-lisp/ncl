use std::collections::HashMap;
use std::rc::Rc;

use ncl_compiler::{
    Constant, DestructurePattern, DestructureSpec, FunctionCode, FunctionId, Instruction, Program,
};
use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::error::ThrowTag;
use crate::evaluator::{ConditionHandlerBinding, RestartBinding};
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

pub(crate) fn run_entry(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    if !function.parameters.is_empty()
        || !function.optional.is_empty()
        || !function.keywords.is_empty()
        || function.has_keyword_section
        || function.rest.is_some()
        || !function.auxiliary.is_empty()
    {
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled entry function")
                .to_string(),
            expected: "0".to_string(),
            actual: 0,
        });
    }
    run_code(runtime, &program, function, environment, span)
}

pub(crate) fn run(
    runtime: &Runtime,
    program: Rc<Program>,
    function_id: FunctionId,
    environment: Environment,
    arguments: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    let required_count = function.parameters.len();
    let optional_count = function.optional.len();
    let maximum_count = required_count + optional_count;
    if arguments.len() < required_count {
        let expected =
            if optional_count > 0 || function.rest.is_some() || function.has_keyword_section {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }
    let optional_supplied_count = if function.has_keyword_section {
        let available = arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count);
        (0..available)
            .take_while(|index| {
                !matches!(
                    arguments[required_count + *index],
                    Value::Keyword(_) | Value::KeywordExact(_)
                )
            })
            .count()
    } else {
        arguments
            .len()
            .saturating_sub(required_count)
            .min(optional_count)
    };
    let key_start = required_count + optional_supplied_count;
    if !function.has_keyword_section && function.rest.is_none() && arguments.len() > maximum_count {
        let expected = if optional_count > 0 {
            format!("at most {maximum_count}")
        } else {
            maximum_count.to_string()
        };
        return Err(RuntimeError::Arity {
            function: function
                .name
                .as_deref()
                .unwrap_or("compiled function")
                .to_string(),
            expected,
            actual: arguments.len(),
        });
    }

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    for (index, (parameter, argument)) in function
        .parameters
        .iter()
        .zip(arguments.iter())
        .enumerate()
    {
        if function.required_escaped.get(index).copied().unwrap_or(false) {
            runtime.define_exact_in(parameter, argument.clone(), &local);
        } else {
            runtime.define_in(parameter, argument.clone(), &local);
        }
    }
    for (index, specification) in function.optional.iter().enumerate() {
        let supplied =
            (index < optional_supplied_count).then(|| &arguments[required_count + index]);
        let value = if let Some(argument) = supplied {
            argument.clone()
        } else {
            let Some(default_function) = program.functions.get(specification.default_function)
            else {
                return Err(RuntimeError::InvalidForm {
                    message: "compiled optional default is out of range".to_string(),
                    span: Some(span),
                });
            };
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
        };
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
        if let Some(supplied_p) = &specification.supplied_p {
            if specification.supplied_p_escaped.unwrap_or(false) {
                runtime.define_exact_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            } else {
                runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
            }
        }
    }
    if let Some(rest) = &function.rest {
        let rest_start = key_start;
        let value = Value::list(arguments[rest_start..].to_vec());
        if function.rest_escaped {
            runtime.define_exact_in(rest, value, &local);
        } else {
            runtime.define_in(rest, value, &local);
        }
    }
    if function.has_keyword_section {
        let keyword_arguments = &arguments[key_start..];
        if keyword_arguments.len() % 2 != 0 {
            return Err(RuntimeError::InvalidForm {
                message: "keyword arguments must be supplied in pairs".to_string(),
                span: Some(span),
            });
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = function.allow_other_keys;
        for pair in keyword_arguments.chunks_exact(2) {
            let keyword = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: "keyword argument name must be a keyword".to_string(),
                        span: Some(span),
                    });
                }
            };
            let keyword_name = keyword.to_string();
            if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                accepts_unknown_keywords = true;
            }
            supplied_keywords.insert(keyword_name, pair[1].clone());
        }
        if !accepts_unknown_keywords {
            for keyword_name in supplied_keywords.keys() {
                if keyword_name != "ALLOW-OTHER-KEYS"
                    && !function
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *keyword_name)
                {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        for specification in &function.keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = if let Some(argument) = supplied {
                argument.clone()
            } else {
                let Some(default_function) = program.functions.get(specification.default_function)
                else {
                    return Err(RuntimeError::InvalidForm {
                        message: "compiled keyword default is out of range".to_string(),
                        span: Some(span),
                    });
                };
                run_code(runtime, &program, default_function, local.clone(), span)?.primary_value()
            };
            if specification.name_escaped {
                runtime.define_exact_in(&specification.name, value, &local);
            } else {
                runtime.define_in(&specification.name, value, &local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                if specification.supplied_p_escaped.unwrap_or(false) {
                    runtime.define_exact_in(
                        supplied_p,
                        Value::boolean(supplied.is_some()),
                        &local,
                    );
                } else {
                    runtime.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                }
            }
        }
    }
    for specification in &function.auxiliary {
        let Some(default_function) = program.functions.get(specification.default_function) else {
            return Err(RuntimeError::InvalidForm {
                message: "compiled auxiliary default is out of range".to_string(),
                span: Some(span),
            });
        };
        let value =
            run_code(runtime, &program, default_function, local.clone(), span)?.primary_value();
        if specification.name_escaped {
            runtime.define_exact_in(&specification.name, value, &local);
        } else {
            runtime.define_in(&specification.name, value, &local);
        }
    }
    run_code(runtime, &program, function, local, span)
}

fn run_code(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    run_code_from(runtime, program, function, environment, span, 0)
}

fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    mut environment: Environment,
    span: Span,
    start_program_counter: usize,
) -> Result<Value, RuntimeError> {
    let mut stack = Vec::new();
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

        match instruction {
            Instruction::Constant(constant) => {
                stack.push(constant_value(constant));
                program_counter += 1;
            }
            Instruction::Quote(form) => {
                stack.push(runtime.quoted_value(form)?);
                program_counter += 1;
            }
            Instruction::QuasiQuote(form) => {
                stack.push(runtime.quasiquote_value(form, &environment)?);
                program_counter += 1;
            }
            Instruction::Load(name) => {
                let value = runtime.lookup_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::LoadExact(name) => {
                let value = runtime.lookup_exact_in(name, &environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionLoad(name) => {
                let value = runtime
                    .lookup_function_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::FunctionLoadExact(name) => {
                let value = runtime
                    .lookup_function_exact_in(name, &environment)
                    .ok_or_else(|| RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    })?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::IsBound(name) => {
                stack.push(Value::boolean(runtime.is_bound_in(name, &environment)));
                program_counter += 1;
            }
            Instruction::IsBoundExact(name) => {
                stack.push(Value::boolean(
                    runtime.lookup_exact_in(name, &environment).is_some(),
                ));
                program_counter += 1;
            }
            Instruction::Define(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_in(name, value.clone(), &environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::DefineExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.define_exact_in(name, value.clone(), &environment);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::DefineFunction(name) => {
                let value = pop_value(&mut stack, span, "local function definition")?;
                environment.define_function(name, value);
                program_counter += 1;
            }
            Instruction::DefineFunctionExact(name) => {
                let value = pop_value(&mut stack, span, "local function definition")?;
                environment.define_function_exact(name, value);
                program_counter += 1;
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
                program_counter += 1;
            }
            Instruction::DefineSpecialExact { name, force } => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
                if *force && runtime.is_constant_exact_in(name) {
                    return Err(runtime.constant_modification_error(name, span));
                }
                let value =
                    runtime.define_special_value_exact(name, value.primary_value(), *force);
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("define-special has no value on the stack", span))? =
                    value;
                program_counter += 1;
            }
            Instruction::DefineValues(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_in(name, value, &environment);
                program_counter += 1;
            }
            Instruction::DefineValuesExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
                runtime.define_exact_in(name, value, &environment);
                program_counter += 1;
            }
            Instruction::Set(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_in(name, value.clone(), &environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::SetExact(name) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_or_define_exact_in(name, value.clone(), &environment, span)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::Setf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_place(place, value.clone(), &environment)?;
                *stack
                    .last_mut()
                    .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
                program_counter += 1;
            }
            Instruction::MapIntoSetf(place) => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("map-into has no value on the stack", span))?;
                let value = value.primary_value();
                runtime.set_map_into_destination(place, value.clone(), &environment)?;
                *stack.last_mut().ok_or_else(|| {
                    invalid("map-into has no value on the stack", span)
                })? = value;
                program_counter += 1;
            }
            Instruction::Psetq(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for (name, value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::PsetqExact(names) => {
                if stack.len() < names.len() {
                    return Err(invalid("psetq has fewer values than targets", span));
                }
                let values = stack.split_off(stack.len() - names.len());
                for ((name, escaped), value) in names.iter().zip(values) {
                    let value = value.primary_value();
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(Value::Nil);
                program_counter += 1;
            }
            Instruction::MultipleValueSetq(names) => {
                let source = pop_value(&mut stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    runtime.set_or_define_in(name, value, &environment, span)?;
                }
                stack.push(source.primary_value());
                program_counter += 1;
            }
            Instruction::MultipleValueSetqExact(names) => {
                let source = pop_value(&mut stack, span, "multiple-value-setq")?;
                let values = source.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.set_or_define_exact_in(name, value, &environment, span)?;
                    } else {
                        runtime.set_or_define_in(name, value, &environment, span)?;
                    }
                }
                stack.push(source.primary_value());
                program_counter += 1;
            }
            Instruction::EnterScope => {
                scopes.push((
                    environment.clone(),
                    runtime.dynamic_depth(),
                    runtime.exact_dynamic_depth(),
                ));
                environment = environment.child();
                program_counter += 1;
            }
            Instruction::ExitScope => {
                let (parent, depth, exact_depth) = scopes
                    .pop()
                    .ok_or_else(|| invalid("scope exit has no matching scope", span))?;
                runtime.truncate_dynamic(depth);
                runtime.truncate_exact_dynamic(exact_depth);
                environment = parent;
                program_counter += 1;
            }
            Instruction::Pop => {
                pop_value(&mut stack, span, "pop")?;
                program_counter += 1;
            }
            Instruction::Dup => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("dup has no value on the stack", span))?;
                stack.push(value);
                program_counter += 1;
            }
            Instruction::Primary => {
                let value = pop_value(&mut stack, span, "primary value")?;
                stack.push(value.primary_value());
                program_counter += 1;
            }
            Instruction::Values(value_count) => {
                if stack.len() < *value_count {
                    return Err(invalid("values has too few stack values", span));
                }
                let values = stack.split_off(stack.len() - *value_count);
                stack.push(Value::values(values));
                program_counter += 1;
            }
            Instruction::MultipleValueList => {
                let value = pop_value(&mut stack, span, "multiple-value-list")?;
                stack.push(Value::list(value.multiple_values()));
                program_counter += 1;
            }
            Instruction::BindValues(names) => {
                let value = pop_value(&mut stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, name) in names.iter().enumerate() {
                    runtime.define_in(
                        name,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        &environment,
                    );
                }
                program_counter += 1;
            }
            Instruction::BindValuesExact(names) => {
                let value = pop_value(&mut stack, span, "multiple-value-bind")?;
                let values = value.multiple_values();
                for (index, (name, escaped)) in names.iter().enumerate() {
                    let value = values.get(index).cloned().unwrap_or(Value::Nil);
                    if *escaped {
                        runtime.define_exact_in(name, value, &environment);
                    } else {
                        runtime.define_in(name, value, &environment);
                    }
                }
                program_counter += 1;
            }
            Instruction::Destructure(specification) => {
                let value = pop_value(&mut stack, span, "destructuring-bind")?;
                destructure_specification(
                    specification,
                    value.primary_value(),
                    runtime,
                    program,
                    &environment,
                    span,
                )?;
                program_counter += 1;
            }
            Instruction::JumpIfFalse(target) => {
                let condition = pop_value(&mut stack, span, "conditional jump")?;
                if condition.is_truthy() {
                    program_counter += 1;
                } else {
                    program_counter = jump_target(function, *target, span)?;
                }
            }
            Instruction::Jump(target) => {
                program_counter = jump_target(function, *target, span)?;
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
                program_counter += 1;
            }
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
                program_counter += 1;
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
                program_counter += 1;
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
                let body_result = run_code(runtime, program, body_function, environment.clone(), span);
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
                program_counter += 1;
            }
            Instruction::RestartBind { body, bindings } => {
                let mut restarts = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let binding_function = program.functions.get(binding.function).ok_or_else(|| {
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
                let body_result = run_code(runtime, program, body_function, environment.clone(), span);
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
                        let Some((_, function)) = restarts.iter().find(|(name, _)| {
                            normalize_name(invoked.as_str()) == *name
                        }) else {
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
                            &environment,
                        )?);
                    }
                }
                program_counter += 1;
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
                program_counter += 1;
            }
            Instruction::WithSimpleRestart { name, body } => {
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled with-simple-restart body id is out of range", span)
                })?;
                let guard = runtime.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
                let body_result = run_code(runtime, program, body_function, environment.clone(), span);
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
                program_counter += 1;
            }
            Instruction::RestartCase { protected, clauses } => {
                let protected_function = program.functions.get(*protected).ok_or_else(|| {
                    invalid("compiled restart-case protected function id is out of range", span)
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
                        let Some(clause) = clauses
                            .iter()
                            .find(|clause| normalize_name(invoked.as_str()) == clause.name.as_str())
                        else {
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
                program_counter += 1;
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

                let guard = runtime.condition_restart_guard(
                    condition_value,
                    restart_values,
                );
                let body_function = program.functions.get(*body).ok_or_else(|| {
                    invalid("compiled with-condition-restarts body id is out of range", span)
                })?;
                let body_result =
                    run_code(runtime, program, body_function, environment.clone(), span);
                drop(guard);
                stack.push(body_result?);
                program_counter += 1;
            }
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
                program_counter += 1;
            }
            Instruction::Throw => {
                let value = pop_value(&mut stack, span, "throw")?;
                let tag = pop_value(&mut stack, span, "throw")?.primary_value();
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
                program_counter += 1;
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
                program_counter += 1;
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
                program_counter += 1;
            }
            Instruction::ReturnFrom { name } => {
                let value = pop_value(&mut stack, span, "return-from")?;
                return Err(RuntimeError::ReturnFrom {
                    block: name.clone(),
                    target: environment.lookup_block(name),
                    value: ReturnValue::new(value),
                    span: Some(span),
                });
            }
            Instruction::Eval(form_span) => {
                let value = pop_value(&mut stack, span, "eval")?.primary_value();
                let form = runtime.form_from_value(&value, *form_span)?;
                stack.push(runtime.eval_values_in(&form, &environment)?);
                program_counter += 1;
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
                    &environment,
                )?);
                program_counter += 1;
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
                    &environment,
                )?);
                program_counter += 1;
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
                                &environment,
                            )?
                            .primary_value(),
                    );
                }
                stack.push(Value::list(results));
                program_counter += 1;
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
                    &environment,
                )?);
                program_counter += 1;
            }
            Instruction::Return => {
                if !scopes.is_empty() {
                    return Err(invalid(
                        "compiled function returned with an open scope",
                        span,
                    ));
                }
                return pop_value(&mut stack, span, "return");
            }
        }
    }
}

fn constant_value(constant: &Constant) -> Value {
    match constant {
        Constant::Nil => Value::Nil,
        Constant::Boolean(value) => Value::boolean(*value),
        Constant::Integer(value) => Value::Integer(*value),
        Constant::Rational {
            numerator,
            denominator,
        } => Value::rational(i128::from(*numerator), i128::from(*denominator))
            .expect("compiler emitted an invalid rational constant"),
        Constant::Float(value) => Value::Float(*value),
        Constant::String(value) => Value::string(value.clone()),
        Constant::Character(value) => Value::Character(*value),
        Constant::Symbol(value) => Value::symbol(value),
        Constant::SymbolExact(value) => Value::symbol_exact(value),
        Constant::Keyword(value) => Value::keyword(value),
        Constant::KeywordExact(value) => Value::keyword_exact(value),
    }
}

fn pop_value(stack: &mut Vec<Value>, span: Span, operation: &str) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| invalid(&format!("{operation} has no value on the stack"), span))
}

fn destructure_specification(
    specification: &DestructureSpec,
    value: Value,
    runtime: &Runtime,
    program: &Rc<Program>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match specification {
        DestructureSpec::Pattern(pattern) => {
            destructure_value(pattern, value, runtime, environment, span)
        }
        DestructureSpec::LambdaList(lambda_list) => {
            let Some(arguments) = value.list_items() else {
                return Err(invalid(
                    "destructuring-bind value must be a proper list",
                    span,
                ));
            };
            if let Some(environment_name) = &lambda_list.environment {
                runtime.define_in(
                    environment_name,
                    Value::environment(environment.clone()),
                    environment,
                );
            }
            if let Some(whole) = &lambda_list.whole {
                runtime.define_in(whole, value.clone(), environment);
            }
            let required_count = lambda_list.required.len();
            let optional_count = lambda_list.optional.len();
            if arguments.len() < required_count {
                return Err(RuntimeError::Arity {
                    function: "destructuring-bind".to_string(),
                    expected: format!("at least {required_count}"),
                    actual: arguments.len(),
                });
            }
            let optional_supplied_count = if lambda_list.has_keyword_section {
                let available = arguments
                    .len()
                    .saturating_sub(required_count)
                    .min(optional_count);
                (0..available)
                    .take_while(|index| {
                        !matches!(
                            arguments[required_count + *index],
                            Value::Keyword(_) | Value::KeywordExact(_)
                        )
                    })
                    .count()
            } else {
                arguments
                    .len()
                    .saturating_sub(required_count)
                    .min(optional_count)
            };
            let key_start = required_count + optional_supplied_count;
            if !lambda_list.has_keyword_section
                && lambda_list.rest.is_none()
                && arguments.len() > required_count + optional_count
            {
                let maximum = required_count + optional_count;
                return Err(RuntimeError::Arity {
                    function: "destructuring-bind".to_string(),
                    expected: format!("at most {maximum}"),
                    actual: arguments.len(),
                });
            }

            for (pattern, argument) in lambda_list
                .required
                .iter()
                .zip(arguments.iter().take(required_count).cloned())
            {
                destructure_value(pattern, argument, runtime, environment, span)?;
            }
            for (index, parameter) in lambda_list.optional.iter().enumerate() {
                let supplied = (index < optional_supplied_count)
                    .then(|| arguments[required_count + index].clone());
                let value = if let Some(argument) = supplied.as_ref() {
                    argument.clone()
                } else {
                    let default_function = program.functions.get(parameter.default_function).ok_or_else(|| {
                        invalid("compiled destructuring optional default is out of range", span)
                    })?;
                    run_code(runtime, program, default_function, environment.clone(), span)?
                        .primary_value()
                };
                destructure_value(&parameter.pattern, value, runtime, environment, span)?;
                if let Some(supplied_p) = &parameter.supplied_p {
                    runtime.define_in(
                        supplied_p,
                        Value::boolean(supplied.is_some()),
                        environment,
                    );
                }
            }
            if let Some(rest_name) = &lambda_list.rest {
                runtime.define_in(
                    rest_name,
                    Value::list(arguments[key_start..].to_vec()),
                    environment,
                );
            }

            if lambda_list.has_keyword_section {
                let keyword_arguments = &arguments[key_start..];
                if keyword_arguments.len() % 2 != 0 {
                    return Err(invalid("keyword arguments must be supplied in pairs", span));
                }
                let mut supplied_keywords = HashMap::new();
                let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
                for pair in keyword_arguments.chunks_exact(2) {
                    let keyword = match &pair[0] {
                        Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                        _ => {
                            return Err(invalid(
                                "keyword argument name must be a keyword",
                                span,
                            ));
                        }
                    };
                    let keyword_name = keyword.to_string();
                    if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                        accepts_unknown_keywords = true;
                    }
                    supplied_keywords.insert(keyword_name, pair[1].clone());
                }
                if !accepts_unknown_keywords {
                    for keyword_name in supplied_keywords.keys() {
                        if keyword_name != "ALLOW-OTHER-KEYS"
                            && !lambda_list
                                .keywords
                                .iter()
                                .any(|parameter| parameter.keyword_name == *keyword_name)
                        {
                            return Err(invalid(
                                &format!("unknown keyword :{keyword_name}"),
                                span,
                            ));
                        }
                    }
                }
                for parameter in &lambda_list.keywords {
                    let supplied = supplied_keywords.get(&parameter.keyword_name);
                    let value = if let Some(argument) = supplied {
                        argument.clone()
                    } else {
                        let default_function =
                            program.functions.get(parameter.default_function).ok_or_else(|| {
                                invalid(
                                    "compiled destructuring keyword default is out of range",
                                    span,
                                )
                            })?;
                        run_code(runtime, program, default_function, environment.clone(), span)?
                            .primary_value()
                    };
                    destructure_value(&parameter.pattern, value, runtime, environment, span)?;
                    if let Some(supplied_p) = &parameter.supplied_p {
                        runtime.define_in(
                            supplied_p,
                            Value::boolean(supplied.is_some()),
                            environment,
                        );
                    }
                }
            }
            for parameter in &lambda_list.auxiliary {
                let default_function =
                    program.functions.get(parameter.default_function).ok_or_else(|| {
                        invalid("compiled destructuring auxiliary default is out of range", span)
                    })?;
                let value =
                    run_code(runtime, program, default_function, environment.clone(), span)?
                        .primary_value();
                runtime.define_in(&parameter.name, value, environment);
            }
            Ok(())
        }
    }
}

fn destructure_value(
    pattern: &DestructurePattern,
    value: Value,
    runtime: &Runtime,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match pattern {
        DestructurePattern::Name(name) => {
            runtime.define_in(name, value, environment);
            Ok(())
        }
        DestructurePattern::List(patterns) => {
            let Some(values) = value.list_items() else {
                return Err(invalid(
                    "destructuring-bind pattern requires a proper list",
                    span,
                ));
            };
            if values.len() != patterns.len() {
                return Err(invalid(
                    "destructuring-bind pattern has the wrong number of elements",
                    span,
                ));
            }
            for (pattern, value) in patterns.iter().zip(values) {
                destructure_value(pattern, value, runtime, environment, span)?;
            }
            Ok(())
        }
        DestructurePattern::Dotted { items, tail } => {
            let Some((values, dotted_tail)) = destructure_dotted_parts(&value) else {
                return Err(invalid("destructuring-bind pattern requires a list", span));
            };
            if values.len() < items.len() {
                return Err(invalid(
                    "destructuring-bind pattern has too few elements",
                    span,
                ));
            }
            for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                destructure_value(pattern, value, runtime, environment, span)?;
            }
            let remaining = values[items.len()..].to_vec();
            let tail_value = if remaining.is_empty() {
                dotted_tail
            } else if dotted_tail.is_truthy() {
                Value::dotted_list(remaining, dotted_tail)
            } else {
                Value::list(remaining)
            };
            destructure_value(tail, tail_value, runtime, environment, span)
        }
    }
}

fn destructure_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = destructure_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}

fn jump_target(function: &FunctionCode, target: usize, span: Span) -> Result<usize, RuntimeError> {
    if target >= function.instructions.len() {
        return Err(invalid("compiled jump target is out of range", span));
    }
    Ok(target)
}

fn invalid(message: &str, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.to_string(),
        span: Some(span),
    }
}

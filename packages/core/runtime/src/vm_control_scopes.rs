#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_restart_bind_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    body: usize,
    bindings: &[RestartBindClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let mut restarts = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let binding_function = program
            .functions
            .get(binding.function)
            .ok_or_else(|| invalid("compiled restart-bind clause id is out of range", span))?;
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
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled restart-bind body id is out of range", span))?;
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    match body_result {
        Ok(value) => stack.push(value),
        Err(error) => {
            let RuntimeError::InvokeRestart {
                name, arguments, ..
            } = &error
            else {
                return Err(error);
            };
            let Some((_, function)) = restarts
                .iter()
                .find(|(restart_name, _)| normalize_name(name.as_str()) == *restart_name)
            else {
                return Err(error);
            };
            let values = arguments
                .iter()
                .cloned()
                .map(ReturnValue::into_value)
                .collect::<Vec<_>>();
            stack.push(runtime.apply_in(function, &values, span, environment)?);
        }
    }
    Ok(())
}

pub(super) fn execute_catch_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    tag: FunctionId,
    body: FunctionId,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let tag_function = program
        .functions
        .get(tag)
        .ok_or_else(|| invalid("compiled catch tag function id is out of range", span))?;
    let tag = run_code(runtime, program, tag_function, environment.clone(), span)?.primary_value();
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled catch body function id is out of range", span))?;
    match run_code(runtime, program, body_function, environment.clone(), span) {
        Ok(value) => stack.push(value),
        Err(RuntimeError::Throw {
            tag: thrown_tag,
            value,
            ..
        }) if thrown_tag.matches(&tag) => stack.push(value.into_value()),
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(super) fn execute_with_simple_restart_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    name: &str,
    body: FunctionId,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled with-simple-restart body id is out of range", span))?;
    let guard = runtime.restart_guard(vec![RestartBinding::new(name.to_string(), None)]);
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    match body_result {
        Ok(value) => stack.push(value),
        Err(RuntimeError::InvokeRestart {
            name: invoked,
            value,
            ..
        }) if normalize_name(invoked.as_str()) == name => stack.push(value.into_value()),
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(super) fn execute_restart_case_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    protected: FunctionId,
    clauses: &[RestartCaseClause],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let protected_function = program.functions.get(protected).ok_or_else(|| {
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
            let Some(clause) = clauses
                .iter()
                .find(|clause| normalize_name(invoked.as_str()) == clause.name.as_str())
            else {
                return Err(error);
            };
            program
                .functions
                .get(clause.function)
                .ok_or_else(|| invalid("compiled restart-case clause id is out of range", span))?;
            let argument_values = arguments
                .iter()
                .cloned()
                .map(ReturnValue::into_value)
                .collect::<Vec<_>>();
            stack.push(run(
                runtime,
                program,
                clause.function,
                environment,
                &argument_values,
                span,
            )?);
        }
    }
    Ok(())
}

pub(super) fn execute_with_condition_restarts_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    functions: (FunctionId, FunctionId, FunctionId),
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let (condition, restarts, body) = functions;
    let condition_function = program.functions.get(condition).ok_or_else(|| {
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
    let restarts_function = program.functions.get(restarts).ok_or_else(|| {
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
    let body_function = program.functions.get(body).ok_or_else(|| {
        invalid(
            "compiled with-condition-restarts body id is out of range",
            span,
        )
    })?;
    let body_result = run_code(runtime, program, body_function, environment.clone(), span);
    drop(guard);
    stack.push(body_result?);
    Ok(())
}

pub(super) fn execute_progv_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    functions: (FunctionId, FunctionId, FunctionId),
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let (symbols, values, body) = functions;
    let symbols_function = program
        .functions
        .get(symbols)
        .ok_or_else(|| invalid("compiled progv symbol function id is out of range", span))?;
    let symbols_value = run_code(
        runtime,
        program,
        symbols_function,
        environment.clone(),
        span,
    )?
    .primary_value();
    let symbol_items = symbols_value
        .list_items()
        .ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: symbols_value.type_name().to_string(),
            span: Some(span),
        })?;
    let values_function = program
        .functions
        .get(values)
        .ok_or_else(|| invalid("compiled progv value function id is out of range", span))?;
    let values_value =
        run_code(runtime, program, values_function, environment.clone(), span)?.primary_value();
    let value_items = values_value
        .list_items()
        .ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: values_value.type_name().to_string(),
            span: Some(span),
        })?;
    let _dynamic_guard = runtime.dynamic_guard();
    for (index, symbol) in symbol_items.iter().enumerate() {
        let name = symbol
            .symbol_name()
            .ok_or_else(|| invalid("progv symbol list must contain only symbols", span))?;
        runtime.define_dynamic(name, value_items.get(index).cloned().unwrap_or(Value::Nil));
    }
    let body_function = program
        .functions
        .get(body)
        .ok_or_else(|| invalid("compiled progv body function id is out of range", span))?;
    stack.push(run_code(
        runtime,
        program,
        body_function,
        environment.clone(),
        span,
    )?);
    Ok(())
}

pub(super) fn execute_block_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    name: &str,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let function = program
        .functions
        .get(function_id)
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
        }) if return_target == target => stack.push(value.into_value()),
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(super) fn execute_tagbody_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    tags: &[(String, usize)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let tagbody_function = program
        .functions
        .get(function_id)
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
                    .ok_or_else(|| invalid("compiled GO target is missing from TAGBODY", span))?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn execute_unwind_protect_instruction(
    runtime: &Runtime,
    program: &Rc<Program>,
    functions: (FunctionId, FunctionId),
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let (protected, cleanup) = functions;
    let protected_function = program.functions.get(protected).ok_or_else(|| {
        invalid(
            "compiled unwind-protect protected function id is out of range",
            span,
        )
    })?;
    let cleanup_function = program.functions.get(cleanup).ok_or_else(|| {
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
    Ok(())
}

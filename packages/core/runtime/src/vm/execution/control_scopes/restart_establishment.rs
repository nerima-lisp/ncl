use std::rc::Rc;

use ncl_compiler::{FunctionId, Program, RestartBindClause};
use ncl_syntax::Span;

use crate::environment::names_equal;
use crate::evaluator::RestartBinding;
use crate::vm::entry::run_code;
use crate::vm::primitives::invalid;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

pub(in crate::vm::execution) fn execute_restart_bind_instruction(
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
                .find(|(restart_name, _)| names_equal(name.as_str(), restart_name))
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

pub(in crate::vm::execution) fn execute_with_simple_restart_instruction(
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
        }) if names_equal(invoked.as_str(), name) => stack.push(value.into_value()),
        Err(error) => return Err(error),
    }
    Ok(())
}

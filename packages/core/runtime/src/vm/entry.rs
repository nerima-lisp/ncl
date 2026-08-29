use std::rc::Rc;

use ncl_compiler::{FunctionCode, FunctionId, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::argument_binding::{
    argument_layout, bind_auxiliary, bind_keywords, bind_optional, bind_required, bind_rest,
};
use super::execution::run_code_from;
use super::primitives::invalid;

pub fn run_entry(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    environment: &Environment,
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
    run_code(runtime, program, function, environment.clone(), span)
}

pub fn run(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    environment: &Environment,
    arguments: &[Value],
    span: Span,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(invalid("compiled function id is out of range", span));
    };
    let (optional_supplied_count, key_start) = argument_layout(function, arguments)?;

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    bind_required(runtime, function, arguments, &local);
    bind_optional(
        runtime,
        program,
        function,
        arguments,
        optional_supplied_count,
        &local,
        span,
    )?;
    bind_rest(runtime, function, arguments, key_start, &local);
    bind_keywords(
        runtime, program, function, arguments, key_start, &local, span,
    )?;
    bind_auxiliary(runtime, program, function, &local, span)?;
    run_code(runtime, program, function, local, span)
}

pub(super) fn run_code(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    run_code_from(runtime, program, function, environment, span, 0)
}

#[cfg(test)]
mod tests;

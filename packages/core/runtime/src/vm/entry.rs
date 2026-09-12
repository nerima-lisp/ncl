use std::rc::Rc;

use ncl_compiler::{Constant, FunctionCode, FunctionId, Instruction, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::argument_binding::{
    BindingContext, argument_layout, bind_auxiliary, bind_keywords, bind_optional, bind_required,
    bind_rest, declare_special_if,
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
    let special_names = leading_special_declaration_names(function);

    let local = environment.child();
    let _dynamic_guard = runtime.dynamic_guard();
    let mut binding_context = BindingContext::new(&local, span, &special_names);
    for (index, parameter) in function.parameters.iter().enumerate() {
        declare_special_if(
            &local,
            parameter,
            function
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false),
            &special_names,
        );
    }
    bind_required(runtime, function, arguments, &local);
    bind_optional(
        runtime,
        program,
        function,
        arguments,
        optional_supplied_count,
        &mut binding_context,
    )?;
    if let Some(rest) = &function.rest {
        binding_context.local = binding_context.local.child();
        declare_special_if(
            &binding_context.local,
            rest,
            function.rest_escaped,
            &special_names,
        );
    }
    bind_rest(
        runtime,
        function,
        arguments,
        key_start,
        &binding_context.local,
    );
    bind_keywords(
        runtime,
        program,
        function,
        arguments,
        key_start,
        &mut binding_context,
    )?;
    bind_auxiliary(runtime, program, function, &mut binding_context)?;
    run_code(
        runtime,
        program,
        function,
        binding_context.local.child(),
        span,
    )
}

fn leading_special_declaration_names(function: &FunctionCode) -> Vec<(String, bool)> {
    let mut names = Vec::new();
    let mut index = 0;
    while index < function.instructions.len() {
        while let Some(instruction) = function.instructions.get(index) {
            match instruction {
                Instruction::DeclareSpecial(name) => {
                    names.push((name.clone(), false));
                    index += 1;
                }
                Instruction::DeclareSpecialExact(name) => {
                    names.push((name.clone(), true));
                    index += 1;
                }
                _ => break,
            }
        }
        if !matches!(
            function.instructions.get(index),
            Some(Instruction::Constant(Constant::Nil))
        ) {
            break;
        }
        index += 1;
        if matches!(function.instructions.get(index), Some(Instruction::Pop)) {
            index += 1;
        } else {
            break;
        }
    }
    names
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

use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::Span;

use crate::vm::entry::run_code;
use crate::vm::execution::run_code_from;
use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(in crate::vm::execution) fn execute_catch_instruction(
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

pub(in crate::vm::execution) fn execute_block_instruction(
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

pub(in crate::vm::execution) fn execute_tagbody_instruction(
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

pub(in crate::vm::execution) fn execute_unwind_protect_instruction(
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

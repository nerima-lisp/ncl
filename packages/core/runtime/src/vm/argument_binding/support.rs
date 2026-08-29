use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::super::entry::run_code;

pub(super) fn default_value(
    runtime: &Runtime,
    program: &Rc<Program>,
    function_id: FunctionId,
    local: &Environment,
    span: Span,
    message: &str,
) -> Result<Value, RuntimeError> {
    let Some(function) = program.functions.get(function_id) else {
        return Err(RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        });
    };
    Ok(run_code(runtime, program, function, local.clone(), span)?.primary_value())
}

pub(super) fn define_binding(
    runtime: &Runtime,
    name: &str,
    value: Value,
    escaped: bool,
    local: &Environment,
) {
    if escaped {
        runtime.define_exact_in(name, value, local);
    } else {
        runtime.define_in(name, value, local);
    }
}

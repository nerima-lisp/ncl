use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::Span;

use crate::vm::entry::run_code;
use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(in crate::vm::execution) fn execute_progv_instruction(
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

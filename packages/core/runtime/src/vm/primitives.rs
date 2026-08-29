use ncl_compiler::{Constant, FunctionCode};
use ncl_syntax::Span;

use crate::{RuntimeError, Value};

pub(super) fn constant_value(constant: &Constant, span: Span) -> Result<Value, RuntimeError> {
    match constant {
        Constant::Nil => Ok(Value::Nil),
        Constant::Boolean(value) => Ok(Value::boolean(*value)),
        Constant::Integer(value) => Ok(Value::Integer(*value)),
        Constant::Rational {
            numerator,
            denominator,
        } => Value::rational(i128::from(*numerator), i128::from(*denominator)).map_err(|_| {
            RuntimeError::InvalidForm {
                message: "compiled rational constant is invalid".to_owned(),
                span: Some(span),
            }
        }),
        Constant::Float(value) => Ok(Value::Float(*value)),
        Constant::String(value) => Ok(Value::string(value.clone())),
        Constant::Character(value) => Ok(Value::Character(*value)),
        Constant::Symbol(value) => Ok(Value::symbol(value)),
        Constant::SymbolExact(value) => Ok(Value::symbol_exact(value)),
        Constant::Keyword(value) => Ok(Value::keyword(value)),
        Constant::KeywordExact(value) => Ok(Value::keyword_exact(value)),
    }
}

pub(super) fn pop_value(
    stack: &mut Vec<Value>,
    span: Span,
    operation: &str,
) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| invalid(&format!("{operation} has no value on the stack"), span))
}

pub(super) fn jump_target(
    function: &FunctionCode,
    target: usize,
    span: Span,
) -> Result<usize, RuntimeError> {
    if target >= function.instructions.len() {
        return Err(invalid("compiled jump target is out of range", span));
    }
    Ok(target)
}

pub(super) fn invalid(message: &str, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.to_string(),
        span: Some(span),
    }
}

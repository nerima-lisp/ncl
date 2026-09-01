#[allow(clippy::wildcard_imports)]
use super::*;

mod equality;
pub use equality::{eql_value, equalp_value};

pub(super) fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

pub fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(!matches!(
        &arguments[0],
        Value::List(_) | Value::MutableCons(_) | Value::DottedList { .. }
    )))
}

pub fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::List(_) | Value::MutableCons(_) | Value::DottedList { .. }
    )))
}

pub fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil | Value::List(_) | Value::MutableCons(_)
    )))
}

pub fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_)
            | Value::BigInteger(_)
            | Value::Rational(_)
            | Value::Float(_)
            | Value::Complex(_)
    )))
}

pub fn complexp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complexp", 1)?;
    Ok(Value::boolean(arguments[0].is_complex()))
}

pub fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::BigInteger(_)
    )))
}

pub fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

pub fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::BigInteger(_) | Value::Rational(_)
    )))
}

pub fn stringp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stringp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_) | Value::MutableString(_))))
}

pub fn simple_string_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-string-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_) | Value::MutableString(_))))
}

pub fn symbolp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbolp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil
            | Value::Boolean(_)
            | Value::Symbol(_)
            | Value::UninternedSymbol(_)
            | Value::Keyword(_)
            | Value::SymbolExact(_)
            | Value::KeywordExact(_)
    )))
}

pub fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Package(_))))
}

pub fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

pub(super) fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

pub(super) fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eql", 2)?;
    Ok(Value::boolean(eql_value(&arguments[0], &arguments[1])))
}

pub(super) fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

pub(super) fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

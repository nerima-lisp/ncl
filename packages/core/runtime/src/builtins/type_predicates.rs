#[allow(clippy::wildcard_imports)]
use super::*;

mod equality;
pub use equality::{eql_value, equalp_value};

pub(crate) fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

pub(crate) fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(!matches!(&arguments[0], Value::Cons(_))))
}

pub(crate) fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Cons(_))))
}

pub(crate) fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil | Value::Cons(_)
    )))
}

pub(crate) fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_)
            | Value::BigInteger(_)
            | Value::Rational(_)
            | Value::BigRational(_)
            | Value::Float(_)
            | Value::Complex(_)
    )))
}

pub(crate) fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::BigInteger(_)
    )))
}

pub(crate) fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

pub(crate) fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::BigInteger(_) | Value::Rational(_) | Value::BigRational(_)
    )))
}

pub(crate) fn realp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realp", 1)?;
    Ok(Value::boolean(arguments[0].is_real_number()))
}

pub(crate) fn complexp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complexp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Complex(_))))
}

pub(crate) fn stringp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stringp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

pub(crate) fn simple_string_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-string-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

pub(crate) fn symbolp(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
            | Value::InternedSymbol(_)
    )))
}

pub(crate) fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Package(_) | Value::PackageObject(_)
    )))
}

pub(crate) fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

pub(crate) fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

pub(crate) fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eql", 2)?;
    Ok(Value::boolean(eql_value(&arguments[0], &arguments[1])))
}

pub(crate) fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

pub(crate) fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

use super::*;

pub(crate) fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

pub(crate) fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

pub(crate) fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub(crate) fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub(crate) fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

pub(crate) fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

pub(crate) fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eql", 2)?;
    Ok(Value::boolean(eql_value(&arguments[0], &arguments[1])))
}

pub(crate) fn eql_value(left: &Value, right: &Value) -> bool {
    let numeric_equal = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::Rational(left), Value::Rational(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left == right,
        _ => false,
    };
    left.eq_value(right) || numeric_equal
}

pub(crate) fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

pub(crate) fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

pub(crate) fn equalp_value(left: &Value, right: &Value) -> bool {
    if let (Ok(left), Ok(right)) = (number(left), number(right)) {
        return numeric_equalp(left, right);
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        (Value::Character(left), Value::Character(right)) => left.eq_ignore_ascii_case(right),
        (Value::List(left), Value::List(right)) | (Value::Vector(left), Value::Vector(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Array {
                dimensions: left_dimensions,
                elements: left_elements,
            },
            Value::Array {
                dimensions: right_dimensions,
                elements: right_elements,
            },
        ) => {
            left_dimensions == right_dimensions
                && left_elements.len() == right_elements.len()
                && left_elements
                    .iter()
                    .zip(right_elements.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::DottedList {
                items: left,
                tail: left_tail,
            },
            Value::DottedList {
                items: right,
                tail: right_tail,
            },
        ) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
                && equalp_value(left_tail, right_tail)
        }
        _ => eql_value(left, right),
    }
}

pub(crate) fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

pub(crate) fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or(arguments[0].type_name()),
    ))
}

pub(crate) fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

pub(crate) fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(!matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

pub(crate) fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

pub(crate) fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil | Value::List(_)
    )))
}

pub(crate) fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_) | Value::Float(_)
    )))
}

pub(crate) fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Integer(_))))
}

pub(crate) fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

pub(crate) fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_)
    )))
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
    )))
}

pub(crate) fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Package(_))))
}

pub(crate) fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

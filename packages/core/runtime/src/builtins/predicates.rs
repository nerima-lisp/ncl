macro_rules! predicate_builtins {
    () => {
fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(
        arguments[0].list_items().is_none()
            && !matches!(&arguments[0], Value::DottedList { .. }),
    ))
}

fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(
        arguments[0]
            .list_items()
            .is_some_and(|items| !items.is_empty())
            || matches!(&arguments[0], Value::DottedList { .. }),
    ))
}

fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(
        matches!(&arguments[0], Value::Nil | Value::Boolean(false))
            || arguments[0].list_items().is_some(),
    ))
}

fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_) | Value::Float(_) | Value::Complex { .. }
    )))
}

fn complexp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complexp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Complex { .. }
    )))
}

fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Integer(_))))
}

fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_)
    )))
}

fn complex(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complex", 2)?;
    Ok(Value::complex(
        real_number_argument("complex", &arguments[0])?,
        real_number_argument("complex", &arguments[1])?,
    ))
}

fn conjugate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "conjugate", 1)?;
    match numeric_argument("conjugate", &arguments[0])? {
        Numeric::Real(value) => number_to_value(value),
        Numeric::Complex { real, imag } => Ok(Value::complex(
            number_to_value(real)?,
            number_to_value(negate_number(imag)?)?,
        )),
    }
}

fn phase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "phase", 1)?;
    match numeric_argument("phase", &arguments[0])? {
        Numeric::Real(value) => phase_real(value),
        Numeric::Complex { real, imag } => phase_complex(real, imag),
    }
}

fn phase_real(value: Number) -> Result<Value, RuntimeError> {
    let as_float = value.as_float();
    if as_float == 0.0 {
        return number_to_value(match value {
            Number::Float(_) => Number::Float(0.0),
            _ => Number::Integer(0),
        });
    }
    if as_float.is_sign_negative() {
        Ok(Value::Float(PI))
    } else {
        number_to_value(match value {
            Number::Float(_) => Number::Float(0.0),
            _ => Number::Integer(0),
        })
    }
}

fn phase_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    if real.as_float() == 0.0 && imag.as_float() == 0.0 {
        return Ok(Value::Integer(0));
    }
    Ok(Value::Float(imag.as_float().atan2(real.as_float())))
}

fn realpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realpart", 1)?;
    match &arguments[0] {
        Value::Complex { real, .. } => Ok(real.as_ref().clone()),
        value if is_real_number(value) => Ok(value.clone()),
        value => Err(number_error("realpart", value)),
    }
}

fn imagpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "imagpart", 1)?;
    match &arguments[0] {
        Value::Complex { imag, .. } => Ok(imag.as_ref().clone()),
        Value::Float(_) => Ok(Value::Float(0.0)),
        value if is_real_number(value) => Ok(Value::Integer(0)),
        value => Err(number_error("imagpart", value)),
    }
}

fn stringp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stringp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn simple_string_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-string-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn symbolp(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Package(_))))
}

fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

fn equalp_value(left: &Value, right: &Value) -> bool {
    if let (Ok(left), Ok(right)) = (number(left), number(right)) {
        return numeric_equalp(left, right);
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        (Value::Character(left), Value::Character(right)) => left.eq_ignore_ascii_case(right),
        (Value::List(left), Value::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Vector {
                fill_pointer: left_fill_pointer,
                element_type: left_element_type,
                adjustable: left_adjustable,
                displaced_to: left_displaced_to,
                displaced_index_offset: left_displaced_index_offset,
                ..
            },
            Value::Vector {
                fill_pointer: right_fill_pointer,
                element_type: right_element_type,
                adjustable: right_adjustable,
                displaced_to: right_displaced_to,
                displaced_index_offset: right_displaced_index_offset,
                ..
            },
        ) => {
            let left = left.vector_items().expect("vector items");
            let right = right.vector_items().expect("vector items");
            left_fill_pointer == right_fill_pointer
                && left_adjustable == right_adjustable
                && left_element_type.equal_value(right_element_type)
                && left_displaced_index_offset == right_displaced_index_offset
                && match (left_displaced_to, right_displaced_to) {
                    (Some(left), Some(right)) => equalp_value(left, right),
                    (None, None) => true,
                    _ => false,
                }
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Array {
                dimensions: left_dimensions,
                element_type: left_element_type,
                adjustable: left_adjustable,
                displaced_to: left_displaced_to,
                displaced_index_offset: left_displaced_index_offset,
                ..
            },
            Value::Array {
                dimensions: right_dimensions,
                element_type: right_element_type,
                adjustable: right_adjustable,
                displaced_to: right_displaced_to,
                displaced_index_offset: right_displaced_index_offset,
                ..
            },
        ) => {
            let left_elements = left.array_items().expect("array items");
            let right_elements = right.array_items().expect("array items");
            left_dimensions == right_dimensions
                && left_adjustable == right_adjustable
                && left_element_type.equal_value(right_element_type)
                && left_displaced_index_offset == right_displaced_index_offset
                && match (left_displaced_to, right_displaced_to) {
                    (Some(left), Some(right)) => equalp_value(left, right),
                    (None, None) => true,
                    _ => false,
                }
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

fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}


    };
}

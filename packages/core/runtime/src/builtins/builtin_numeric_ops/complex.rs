use crate::builtins::builtin_helpers::number_error;

use super::*;

fn real_argument(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    if value.is_real_number() {
        Ok(value.clone())
    } else {
        Err(type_error(function, "real", value))
    }
}

fn parts(function: &str, value: &Value) -> Result<(Value, Value), RuntimeError> {
    match value {
        Value::Complex(value) => Ok((value.real().clone(), value.imaginary().clone())),
        value if value.is_real_number() => Ok((value.clone(), Value::Integer(0))),
        value => Err(number_error(function, value)),
    }
}

fn real_binary(
    function: &str,
    left: &Value,
    right: &Value,
    operator: char,
) -> Result<Value, RuntimeError> {
    let left = number_argument(function, left)?;
    let right = number_argument(function, right)?;
    if left.is_float() || right.is_float() {
        let left = left.as_float();
        let right = right.as_float();
        let result = match operator {
            '+' => left + right,
            '-' => left - right,
            '*' => left * right,
            '/' => {
                if right == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                left / right
            }
            _ => unreachable!("unsupported real complex operator"),
        };
        Ok(Value::Float(result))
    } else {
        number_to_value(exact_binary(&left, &right, operator)?)
    }
}

fn negate_real(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    number_to_value(negate_number(number_argument(function, value)?)?)
}

fn make_complex(real: Value, imaginary: Value) -> Value {
    Value::complex(real, imaginary)
}

fn complex_binary(
    function: &str,
    left: &Value,
    right: &Value,
    operator: char,
) -> Result<Value, RuntimeError> {
    let (left_real, left_imaginary) = parts(function, left)?;
    let (right_real, right_imaginary) = parts(function, right)?;
    let (real, imaginary) = match operator {
        '+' => (
            real_binary(function, &left_real, &right_real, '+')?,
            real_binary(function, &left_imaginary, &right_imaginary, '+')?,
        ),
        '-' => (
            real_binary(function, &left_real, &right_real, '-')?,
            real_binary(function, &left_imaginary, &right_imaginary, '-')?,
        ),
        '*' => {
            let real = real_binary(
                function,
                &real_binary(function, &left_real, &right_real, '*')?,
                &real_binary(function, &left_imaginary, &right_imaginary, '*')?,
                '-',
            )?;
            let imaginary = real_binary(
                function,
                &real_binary(function, &left_real, &right_imaginary, '*')?,
                &real_binary(function, &left_imaginary, &right_real, '*')?,
                '+',
            )?;
            (real, imaginary)
        }
        '/' => {
            let denominator = real_binary(
                function,
                &real_binary(function, &right_real, &right_real, '*')?,
                &real_binary(function, &right_imaginary, &right_imaginary, '*')?,
                '+',
            )?;
            let real = real_binary(
                function,
                &real_binary(function, &left_real, &right_real, '*')?,
                &real_binary(function, &left_imaginary, &right_imaginary, '*')?,
                '+',
            )?;
            let imaginary = real_binary(
                function,
                &real_binary(function, &left_imaginary, &right_real, '*')?,
                &real_binary(function, &left_real, &right_imaginary, '*')?,
                '-',
            )?;
            (
                real_binary(function, &real, &denominator, '/')?,
                real_binary(function, &imaginary, &denominator, '/')?,
            )
        }
        _ => unreachable!("unsupported complex operator"),
    };
    Ok(make_complex(real, imaginary))
}

pub(crate) fn complex_add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Value::Integer(0);
    for argument in arguments {
        result = complex_binary("+", &result, argument, '+')?;
    }
    Ok(result)
}

pub(crate) fn complex_subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (first, rest) = arguments
        .split_first()
        .ok_or_else(|| arity("-", "at least one", 0))?;
    if rest.is_empty() {
        let (real, imaginary) = parts("-", first)?;
        return Ok(make_complex(
            negate_real("-", &real)?,
            negate_real("-", &imaginary)?,
        ));
    }
    let mut result = first.clone();
    for argument in rest {
        result = complex_binary("-", &result, argument, '-')?;
    }
    Ok(result)
}

pub(crate) fn complex_multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Value::Integer(1);
    for argument in arguments {
        result = complex_binary("*", &result, argument, '*')?;
    }
    Ok(result)
}

pub(crate) fn complex_divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (first, rest) = arguments
        .split_first()
        .ok_or_else(|| arity("/", "at least one", 0))?;
    let mut result = if rest.is_empty() {
        Value::Integer(1)
    } else {
        first.clone()
    };
    if rest.is_empty() {
        return complex_binary("/", &result, first, '/');
    }
    for argument in rest {
        result = complex_binary("/", &result, argument, '/')?;
    }
    Ok(result)
}

pub fn complex(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("complex", "1 or 2", arguments.len()));
    }
    let real = real_argument("complex", &arguments[0])?;
    let imaginary = arguments
        .get(1)
        .map(|value| real_argument("complex", value))
        .transpose()?
        .unwrap_or(Value::Integer(0));
    Ok(Value::complex(real, imaginary))
}

pub fn realpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realpart", 1)?;
    match &arguments[0] {
        Value::Complex(value) => Ok(value.real().clone()),
        value if value.is_real_number() => Ok(value.clone()),
        value => Err(number_error("realpart", value)),
    }
}

pub fn imagpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "imagpart", 1)?;
    match &arguments[0] {
        Value::Complex(value) => Ok(value.imaginary().clone()),
        Value::Float(_) => Ok(Value::Float(0.0)),
        value if value.is_real_number() => Ok(Value::Integer(0)),
        value => Err(number_error("imagpart", value)),
    }
}

pub fn conjugate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "conjugate", 1)?;
    match &arguments[0] {
        Value::Complex(value) => {
            let imaginary = number_to_value(negate_number(number_argument(
                "conjugate",
                value.imaginary(),
            )?)?)?;
            Ok(Value::complex(value.real().clone(), imaginary))
        }
        value if value.is_real_number() => Ok(value.clone()),
        value => Err(number_error("conjugate", value)),
    }
}

pub fn phase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "phase", 1)?;
    match &arguments[0] {
        Value::Complex(value) => {
            let real = number_argument("phase", value.real())?.as_float();
            let imaginary = number_argument("phase", value.imaginary())?.as_float();
            Ok(Value::Float(imaginary.atan2(real)))
        }
        value => {
            let value = number_argument("phase", value)?.as_float();
            let phase = if value < 0.0 || (value == 0.0 && value.is_sign_negative()) {
                std::f64::consts::PI
            } else {
                0.0
            };
            Ok(Value::Float(phase))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_handles_negative_zero_and_complex_values() {
        let negative_zero = match phase(&[Value::Float(-0.0)]) {
            Ok(Value::Float(value)) => value,
            result => panic!("expected float phase, got {result:?}"),
        };
        assert_eq!(negative_zero, std::f64::consts::PI);

        let complex = Value::complex(Value::Integer(3), Value::Integer(4));
        let phase = match phase(&[complex]) {
            Ok(Value::Float(value)) => value,
            result => panic!("expected float phase, got {result:?}"),
        };
        assert_eq!(phase, 4.0f64.atan2(3.0));
    }
}

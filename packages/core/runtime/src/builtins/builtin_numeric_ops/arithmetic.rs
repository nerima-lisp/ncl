use super::{
    Number, RuntimeError, Value, arity, exact, exact_binary, negate_number, number_argument,
    number_to_value,
};

pub fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.iter().any(Value::is_complex) {
        return super::complex_add(arguments);
    }
    let mut result = Number::Integer(0);
    for argument in arguments {
        let value = number_argument("+", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() + value.as_float())
        } else {
            exact_binary(&result, &value, '+')?
        };
    }
    number_to_value(result)
}

pub fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    if arguments.iter().any(Value::is_complex) {
        return super::complex_subtract(arguments);
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0].clone();
    if values.len() == 1 {
        result = negate_number(result)?;
    } else {
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                Number::Float(result.as_float() - value.as_float())
            } else {
                exact_binary(&result, value, '-')?
            };
        }
    }
    number_to_value(result)
}

pub fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.iter().any(Value::is_complex) {
        return super::complex_multiply(arguments);
    }
    let mut result = Number::Integer(1);
    for argument in arguments {
        let value = number_argument("*", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() * value.as_float())
        } else {
            exact_binary(&result, &value, '*')?
        };
    }
    number_to_value(result)
}

pub fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/", "at least one", 0));
    }
    if arguments.iter().any(Value::is_complex) {
        return super::complex_divide(arguments);
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("/", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result;
    if values.len() == 1 {
        result = if values[0].is_float() {
            let divisor = values[0].as_float();
            if divisor == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Number::Float(1.0 / divisor)
        } else {
            exact_binary(&Number::Integer(1), &values[0], '/')?
        };
    } else {
        result = values[0].clone();
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                let divisor = value.as_float();
                if divisor == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Number::Float(result.as_float() / divisor)
            } else {
                exact_binary(&result, value, '/')?
            };
        }
    }
    number_to_value(result)
}

pub fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

pub fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

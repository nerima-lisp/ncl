use super::{
    Number, RuntimeError, Value, arity, exact, exact_binary, negate_number, number_argument,
    number_to_value,
};

pub fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_unary_and_zero_arity_arithmetic() {
        assert_eq!(numeric_result(increment(&[Value::Integer(4)])), "5");
        assert_eq!(numeric_result(decrement(&[Value::Integer(4)])), "3");
        assert_eq!(numeric_result(subtract(&[Value::Integer(4)])), "-4");
        assert_eq!(numeric_result(add(&[])), "0");
        assert_eq!(numeric_result(multiply(&[])), "1");
    }

    #[test]
    fn rejects_invalid_arithmetic_arguments() {
        assert!(increment(&[]).is_err());
        assert!(subtract(&[]).is_err());
        assert!(divide(&[]).is_err());
    }

    #[test]
    fn promotes_to_float_when_any_argument_is_float() {
        assert_eq!(
            numeric_result(add(&[Value::Integer(1), Value::Float(2.5)])),
            "3.5"
        );
        assert_eq!(
            numeric_result(subtract(&[Value::Integer(5), Value::Float(1.5)])),
            "3.5"
        );
        assert_eq!(
            numeric_result(multiply(&[Value::Integer(2), Value::Float(1.5)])),
            "3.0"
        );
    }

    #[test]
    fn divides_single_float_argument_as_reciprocal() {
        assert_eq!(numeric_result(divide(&[Value::Float(2.0)])), "0.5");
        assert!(matches!(
            divide(&[Value::Float(0.0)]),
            Err(RuntimeError::DivisionByZero)
        ));
    }

    #[test]
    fn divides_float_arguments_and_rejects_float_division_by_zero() {
        assert_eq!(
            numeric_result(divide(&[Value::Float(6.0), Value::Float(2.0)])),
            "3.0"
        );
        assert!(matches!(
            divide(&[Value::Float(1.0), Value::Float(0.0)]),
            Err(RuntimeError::DivisionByZero)
        ));
    }

    fn numeric_result(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("unexpected numeric error: {error}"),
        }
    }
}

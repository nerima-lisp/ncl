use std::cmp::Ordering;

use super::{
    Number, RuntimeError, Value, arity, compare_number_values, exact, number_argument,
    number_to_value, rational_number,
};

pub fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("=", arguments, |ordering| ordering == Ordering::Equal)
}

pub fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

pub fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

pub fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

pub fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

pub fn compare_numbers(
    function: &str,
    arguments: &[Value],
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(values.windows(2).all(|window| {
        comparison(compare_number_values(window[0], window[1]))
    })))
}

pub fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

pub fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

pub fn extreme(
    arguments: &[Value],
    function: &str,
    choose_minimum: bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    for value in &values[1..] {
        let ordering = compare_number_values(*value, result);
        if (choose_minimum && ordering == Ordering::Less)
            || (!choose_minimum && ordering == Ordering::Greater)
        {
            result = *value;
        }
    }
    number_to_value(result)
}

pub fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match number_argument("abs", &arguments[0])? {
        Number::Integer(value) => value
            .checked_abs()
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => number_to_value(rational_number(
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_and_bounds_numbers() {
        assert_eq!(
            numeric_result(compare_numbers(
                "<=",
                &[Value::Integer(1), Value::Integer(1)],
                |ordering| { ordering != Ordering::Greater }
            )),
            "T",
        );
        assert_eq!(
            numeric_result(compare_numbers(
                ">",
                &[Value::Integer(3), Value::Integer(2)],
                |ordering| { ordering == Ordering::Greater }
            )),
            "T",
        );
        assert_eq!(numeric_result(absolute(&[Value::Integer(-4)])), "4");
        assert_eq!(
            numeric_result(minimum(&[Value::Integer(3), Value::Integer(1)])),
            "1",
        );
        assert_eq!(
            numeric_result(maximum(&[Value::Integer(3), Value::Integer(1)])),
            "3",
        );
    }

    #[test]
    fn rejects_invalid_comparison_arguments() {
        assert!(compare_numbers("<", &[], |_| true).is_err());
        assert!(absolute(&[Value::Nil]).is_err());
        assert!(minimum(&[]).is_err());
    }

    #[test]
    fn public_comparison_predicates_delegate_correctly() {
        assert_eq!(
            numeric_result(greater_than(&[Value::Integer(3), Value::Integer(2)])),
            "T",
        );
        assert_eq!(
            numeric_result(greater_than(&[Value::Integer(2), Value::Integer(3)])),
            "NIL",
        );
        assert_eq!(
            numeric_result(less_equal(&[Value::Integer(1), Value::Integer(1)])),
            "T",
        );
        assert_eq!(
            numeric_result(less_equal(&[Value::Integer(2), Value::Integer(1)])),
            "NIL",
        );
        assert_eq!(
            numeric_result(greater_equal(&[Value::Integer(1), Value::Integer(1)])),
            "T",
        );
        assert_eq!(
            numeric_result(greater_equal(&[Value::Integer(1), Value::Integer(2)])),
            "NIL",
        );
    }

    #[test]
    fn absolute_handles_rational_and_float_values() {
        let negative_half =
            Value::rational(-1, 2).unwrap_or_else(|error| panic!("valid rational: {error}"));
        assert_eq!(numeric_result(absolute(&[negative_half])), "1/2");
        assert_eq!(numeric_result(absolute(&[Value::Float(-2.5)])), "2.5");
    }

    fn numeric_result(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("unexpected numeric error: {error}"),
        }
    }
}

use std::cmp::Ordering;

use super::{
    Number, RuntimeError, Value, arity, big_rational_number, compare_number_values, exact,
    number_argument, number_to_value, rational_number,
};

pub fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("=", "at least one", 0));
    }
    for pair in arguments.windows(2) {
        if !numeric_values_equal(&pair[0], &pair[1])? {
            return Ok(Value::boolean(false));
        }
    }
    Ok(Value::boolean(true))
}

fn numeric_values_equal(left: &Value, right: &Value) -> Result<bool, RuntimeError> {
    let (left_real, left_imaginary) = numeric_parts("=", left)?;
    let (right_real, right_imaginary) = numeric_parts("=", right)?;
    Ok(
        compare_number_values(&left_real, &right_real) == Ordering::Equal
            && compare_number_values(&left_imaginary, &right_imaginary) == Ordering::Equal,
    )
}

fn numeric_parts(function: &str, value: &Value) -> Result<(Number, Number), RuntimeError> {
    match value {
        Value::Complex(value) => Ok((
            number_argument(function, value.real())?,
            number_argument(function, value.imaginary())?,
        )),
        value => Ok((number_argument(function, value)?, Number::Integer(0))),
    }
}

pub fn numeric_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/=", "at least one", 0));
    }
    for (index, left) in arguments.iter().enumerate() {
        for right in arguments.iter().skip(index + 1) {
            if numeric_values_equal(left, right)? {
                return Ok(Value::boolean(false));
            }
        }
    }
    Ok(Value::boolean(true))
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
        comparison(compare_number_values(&window[0], &window[1]))
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
    // Track the extremum by index rather than cloning on every improving
    // comparison, so an n-argument call clones once (at the end) instead of
    // up to n times -- the difference matters once bignums are involved,
    // where a clone is a real heap allocation rather than a bitwise copy.
    let mut extreme_index = 0;
    for (index, value) in values.iter().enumerate().skip(1) {
        let ordering = compare_number_values(value, &values[extreme_index]);
        if (choose_minimum && ordering == Ordering::Less)
            || (!choose_minimum && ordering == Ordering::Greater)
        {
            extreme_index = index;
        }
    }
    number_to_value(values[extreme_index].clone())
}

pub fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    if let Value::Complex(value) = &arguments[0] {
        let real = number_argument("abs", value.real())?.as_float();
        let imaginary = number_argument("abs", value.imaginary())?.as_float();
        return Ok(Value::Float(real.hypot(imaginary)));
    }
    match number_argument("abs", &arguments[0])? {
        Number::Integer(value) => Ok(value.checked_abs().map_or_else(
            // i64::MIN is the one integer whose absolute value doesn't fit
            // back in i64 -- promote rather than erroring, consistent with
            // every other exact-arithmetic overflow in this codebase.
            || Value::big_integer(-ibig::IBig::from(value)),
            Value::Integer,
        )),
        Number::Big(value) => Ok(Value::big_integer(ibig::ops::Abs::abs(value))),
        Number::Rational(value) => number_to_value(rational_number(
            value.numerator_i128().unwrap_or(0).abs(),
            value.denominator_i128().unwrap_or(1),
        )?),
        Number::BigRational(value) => number_to_value(big_rational_number(
            ibig::ops::Abs::abs(value.numerator()),
            value.denominator().clone(),
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

    #[test]
    fn absolute_handles_complex_values() {
        let value = Value::complex(Value::Integer(3), Value::Integer(4));
        assert_eq!(numeric_result(absolute(&[value])), "5.0");
    }

    fn numeric_result(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("unexpected numeric error: {error}"),
        }
    }
}

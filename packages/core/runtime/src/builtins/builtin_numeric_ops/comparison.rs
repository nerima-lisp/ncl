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
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
    }
}

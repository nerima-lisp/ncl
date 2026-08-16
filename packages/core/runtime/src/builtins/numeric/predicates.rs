fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("=", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("=", value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(
        values
            .windows(2)
            .all(|window| numeric_equal_values(window[0], window[1])),
    ))
}

fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        match numeric_argument("zerop", &arguments[0])? {
            Numeric::Real(number) => number.as_float() == 0.0,
            Numeric::Complex { real, imag } => real.as_float() == 0.0 && imag.as_float() == 0.0,
        },
    ))
}

fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

fn extreme(
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

fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match numeric_argument("abs", &arguments[0])? {
        Numeric::Real(number) => absolute_real(number),
        Numeric::Complex { real, imag } => absolute_complex(real, imag),
    }
}

fn absolute_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
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

fn absolute_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    let magnitude_squared =
        add_numbers(multiply_numbers(real, real)?, multiply_numbers(imag, imag)?)?;
    square_root_real(magnitude_squared)
}

fn compare_numbers(
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


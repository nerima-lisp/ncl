fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

fn exact_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let (dividend_numerator, dividend_denominator) = dividend
        .exact_parts()
        .expect("exact quotient received a float");
    let (divisor_numerator, divisor_denominator) = divisor
        .exact_parts()
        .expect("exact quotient received a float");
    if divisor_numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }

    let dividend_numerator = i128::from(dividend_numerator);
    let dividend_denominator = i128::from(dividend_denominator);
    let divisor_numerator = i128::from(divisor_numerator);
    let divisor_denominator = i128::from(divisor_denominator);
    let mut quotient_numerator = dividend_numerator * divisor_denominator;
    let mut quotient_denominator = dividend_denominator * divisor_numerator;
    if quotient_denominator < 0 {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = quotient_numerator / quotient_denominator;
    let quotient =
        adjust_exact_quotient(truncated, quotient_numerator, quotient_denominator, mode)?;
    let quotient = i64::try_from(quotient).map_err(|_| RuntimeError::NumericOverflow)?;
    let remainder = rational_number(
        dividend_numerator * divisor_denominator
            - i128::from(quotient) * divisor_numerator * dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        Value::Integer(quotient),
        number_to_value(remainder)?,
    ]))
}

fn adjust_exact_quotient(
    truncated: i128,
    numerator: i128,
    denominator: i128,
    mode: RoundingMode,
) -> Result<i128, RuntimeError> {
    let remainder = numerator % denominator;
    if remainder == 0 {
        return Ok(truncated);
    }
    let direction = if numerator < 0 { -1 } else { 1 };
    match mode {
        RoundingMode::Truncate => Ok(truncated),
        RoundingMode::Floor if direction < 0 => truncated
            .checked_sub(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Ceiling if direction > 0 => truncated
            .checked_add(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > denominator || (distance == denominator && truncated % 2 != 0) {
                truncated
                    .checked_add(direction)
                    .ok_or(RuntimeError::NumericOverflow)
            } else {
                Ok(truncated)
            }
        }
        _ => Ok(truncated),
    }
}

fn float_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let dividend = dividend.as_float();
    let divisor = divisor.as_float();
    if divisor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let ratio = dividend / divisor;
    let rounded = match mode {
        RoundingMode::Floor => ratio.floor(),
        RoundingMode::Ceiling => ratio.ceil(),
        RoundingMode::Truncate => ratio.trunc(),
        RoundingMode::Round => round_float(ratio),
    };
    let quotient = float_integer(rounded)?;
    let remainder = Value::Float(dividend - quotient as f64 * divisor);
    Ok(Value::values(vec![Value::Integer(quotient), remainder]))
}

fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 1i128;
    for argument in arguments {
        let value = i128::from(integer_argument("lcm", argument)?);
        if result == 0 || value == 0 {
            result = 0;
            continue;
        }
        let divisor = integer_gcd(result, value);
        result = (result / divisor)
            .checked_mul(value.abs())
            .ok_or(RuntimeError::NumericOverflow)?;
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}


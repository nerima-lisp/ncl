use super::*;

pub(crate) fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(0);
    for argument in arguments {
        let value = number_argument("+", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() + value.as_float())
        } else {
            exact_binary(result, value, '+')?
        };
    }
    number_to_value(result)
}

pub(crate) fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    if values.len() == 1 {
        result = negate_number(result)?;
    } else {
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                Number::Float(result.as_float() - value.as_float())
            } else {
                exact_binary(result, *value, '-')?
            };
        }
    }
    number_to_value(result)
}

pub(crate) fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(1);
    for argument in arguments {
        let value = number_argument("*", argument)?;
        result = if result.is_float() || value.is_float() {
            Number::Float(result.as_float() * value.as_float())
        } else {
            exact_binary(result, value, '*')?
        };
    }
    number_to_value(result)
}

pub(crate) fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
            exact_binary(Number::Integer(1), values[0], '/')?
        };
    } else {
        result = values[0];
        for value in &values[1..] {
            result = if result.is_float() || value.is_float() {
                let divisor = value.as_float();
                if divisor == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Number::Float(result.as_float() / divisor)
            } else {
                exact_binary(result, *value, '/')?
            };
        }
    }
    number_to_value(result)
}

pub(crate) fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if !base.is_float()
        && let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts()
        && exponent_denominator == 1
    {
        return number_to_value(exact_power(base, exponent_numerator)?);
    }

    Ok(Value::Float(base.as_float().powf(exponent.as_float())))
}

pub(crate) fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match number_argument("sqrt", &arguments[0])? {
        Number::Integer(value) if value >= 0 => {
            let root = integer_square_root(value as u128);
            if root * root == value as u128 {
                Ok(Value::Integer(root as i64))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Integer(_) => Err(negative_real_error("sqrt")),
        Number::Rational(value) if value.numerator() >= 0 => {
            let numerator = value.numerator() as u128;
            let denominator = value.denominator() as u128;
            let numerator_root = integer_square_root(numerator);
            let denominator_root = integer_square_root(denominator);
            if numerator_root * numerator_root == numerator
                && denominator_root * denominator_root == denominator
            {
                rational_number(numerator_root as i128, denominator_root as i128)
                    .and_then(number_to_value)
            } else {
                Ok(Value::Float(
                    (value.numerator() as f64 / value.denominator() as f64).sqrt(),
                ))
            }
        }
        Number::Rational(_) => Err(negative_real_error("sqrt")),
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Float(_) => Err(negative_real_error("sqrt")),
    }
}

pub(crate) fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bits = 128 - value.leading_zeros();
    let mut root = 1u128 << (bits / 2 + 1);
    loop {
        let next = (root + value / root) / 2;
        if next >= root {
            return root;
        }
        root = next;
    }
}

pub(crate) fn negative_real_error(function: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} of a negative real requires complex numbers"),
        span: None,
    }
}

pub(crate) fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match number_argument("signum", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Rational(value) => Ok(Value::Integer(value.numerator().signum())),
        Number::Float(value) if value.is_nan() => Err(RuntimeError::InvalidForm {
            message: "signum of NaN is undefined".to_owned(),
            span: None,
        }),
        Number::Float(value) if value == 0.0 => Ok(Value::Float(value)),
        Number::Float(value) => Ok(Value::Float(if value.is_sign_negative() {
            -1.0
        } else {
            1.0
        })),
    }
}

pub(crate) fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1)
        && !matches!(prototype, Value::Float(_))
    {
        return Err(type_error("float", "a float prototype", prototype));
    }
    Ok(Value::Float(number.as_float()))
}

pub(crate) fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
    }
}

pub(crate) fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rational requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    const FRACTION_MASK: u64 = (1 << 52) - 1;
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let mut significand = bits & FRACTION_MASK;
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        significand |= 1 << 52;
        exponent_bits - 1023 - 52
    };

    if exponent < 0 {
        let canceled = significand.trailing_zeros().min((-exponent) as u32);
        significand >>= canceled;
        exponent += canceled as i32;
    }

    let mut numerator = i128::from(significand);
    if negative {
        numerator = -numerator;
    }
    let denominator = if exponent >= 0 {
        numerator = numerator
            .checked_shl(exponent as u32)
            .ok_or(RuntimeError::NumericOverflow)?;
        1
    } else {
        1i128
            .checked_shl((-exponent) as u32)
            .ok_or(RuntimeError::NumericOverflow)?
    };
    Value::rational(numerator, denominator)
}

pub(crate) fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalize", 1)?;
    match number_argument("rationalize", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rationalize_float(value),
    }
}

pub(crate) fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rationalize requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    let tolerance = (value.abs() * f64::EPSILON / 2.0).max(f64::MIN_POSITIVE);
    let (numerator, denominator) = simplest_rational(value - tolerance, value + tolerance)?;
    number_to_value(rational_number(numerator, denominator)?)
}

pub(crate) fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }
    if lower <= 0.0 && upper >= 0.0 {
        return Ok((0, 1));
    }
    if upper < 0.0 {
        let (numerator, denominator) = simplest_positive_rational(-upper, -lower, 0)?;
        return Ok((-numerator, denominator));
    }
    simplest_positive_rational(lower, upper, 0)
}

pub(crate) fn simplest_positive_rational(
    lower: f64,
    upper: f64,
    depth: u32,
) -> Result<(i128, i128), RuntimeError> {
    if depth > 128 || !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }

    let lower_floor = lower.floor();
    let upper_floor = upper.floor();
    if lower == lower_floor {
        return Ok((lower_floor as i128, 1));
    }
    if lower_floor < upper_floor {
        return Ok(((lower_floor as i128) + 1, 1));
    }

    let lower_fraction = lower - lower_floor;
    let upper_fraction = upper - lower_floor;
    let (reciprocal_numerator, reciprocal_denominator) =
        simplest_positive_rational(1.0 / upper_fraction, 1.0 / lower_fraction, depth + 1)?;
    let numerator = (lower_floor as i128)
        .checked_mul(reciprocal_numerator)
        .and_then(|value| value.checked_add(reciprocal_denominator))
        .ok_or(RuntimeError::NumericOverflow)?;
    Ok((numerator, reciprocal_numerator))
}

pub(crate) fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
    let (mut numerator, mut denominator) =
        base.exact_parts().expect("exact power received a float");
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    rational_number(
        checked_power(i128::from(numerator), magnitude)?,
        checked_power(i128::from(denominator), magnitude)?,
    )
}

pub(crate) fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
    let mut result = 1i128;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = factor
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
    }
    Ok(result)
}

pub(crate) fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("=", arguments, |ordering| ordering == Ordering::Equal)
}

pub(crate) fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

pub(crate) fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

pub(crate) fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

pub(crate) fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

pub(crate) fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        number_argument("zerop", &arguments[0])?.as_float() == 0.0,
    ))
}

pub(crate) fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

pub(crate) fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

pub(crate) fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

pub(crate) fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

pub(crate) fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

pub(crate) fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

pub(crate) fn extreme(
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

pub(crate) fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn compare_numbers(
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

pub(crate) fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

pub(crate) fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
pub(crate) enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

pub(crate) fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

pub(crate) fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

pub(crate) fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

pub(crate) fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

pub(crate) fn quotient_and_remainder(
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

pub(crate) fn exact_quotient_and_remainder(
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

pub(crate) fn adjust_exact_quotient(
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

pub(crate) fn float_quotient_and_remainder(
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

pub(crate) fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

pub(crate) fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

pub(crate) fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

pub(crate) fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(crate) fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

pub(crate) fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

pub(crate) fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = integer_argument("mod", &arguments[0])?;
    let right = integer_argument("mod", &arguments[1])?;
    let remainder = integer_remainder(left, right)?;
    if remainder != 0 && (left < 0) != (right < 0) {
        remainder
            .checked_add(right)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow)
    } else {
        Ok(Value::Integer(remainder))
    }
}

pub(crate) fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

pub(crate) fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

pub(crate) fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        if count >= 64 {
            return if value == 0 {
                Ok(Value::Integer(0))
            } else {
                Err(RuntimeError::NumericOverflow)
            };
        }
        return value
            .checked_shl(count as u32)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        (-count) as u64
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 { -1 } else { 0 }
    } else {
        value >> shift as u32
    }))
}

pub(crate) fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

pub(crate) fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

pub(crate) fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

pub(crate) fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: i64,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_argument(function, argument)?);
    }
    Ok(Value::Integer(result))
}

pub(crate) fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

pub(crate) fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

pub(crate) fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(count as i64))
}

pub(crate) fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < 0 { !value } else { value } as u64;
    Ok(Value::Integer((64 - magnitude.leading_zeros()) as i64))
}

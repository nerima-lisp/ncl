fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
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

fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
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

fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
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

fn simplest_positive_rational(
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

fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
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

fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
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


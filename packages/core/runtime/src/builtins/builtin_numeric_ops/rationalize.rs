use super::{
    Number, RuntimeError, Value, exact, number_argument, number_to_value, rational_number,
};

pub fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
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

pub fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
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

#[expect(
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    reason = "finite validated bounds make these conversions exact for the rational search"
)]
pub fn simplest_positive_rational(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T: std::fmt::Debug>(result: Result<T, RuntimeError>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn finds_simplest_rational_approximation() {
        assert_eq!(ok(rationalize(&[Value::Float(0.5)])).to_string(), "1/2");
        assert_eq!(ok(simplest_rational(0.4, 0.6)), (1, 2));
        assert_eq!(ok(simplest_rational(-0.1, 0.1)), (0, 1));
    }

    #[test]
    fn rejects_invalid_rationalize_arguments() {
        assert!(rationalize(&[]).is_err());
        assert!(simplest_rational(0.6, 0.4).is_err());
        assert!(rationalize_float(f64::NAN).is_err());
    }
}

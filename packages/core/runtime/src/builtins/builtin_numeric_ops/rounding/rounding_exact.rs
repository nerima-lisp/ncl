use super::super::{Number, RuntimeError, Value};
use super::RoundingMode;

use ibig::IBig;

mod float_quotient;
pub use float_quotient::float_quotient_and_remainder;

pub fn exact_quotient_and_remainder(
    dividend: &Number,
    divisor: &Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let Some((dividend_numerator, dividend_denominator)) = exact_parts(dividend) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float".to_string(),
            span: None,
        });
    };
    let Some((divisor_numerator, divisor_denominator)) = exact_parts(divisor) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float".to_string(),
            span: None,
        });
    };
    if divisor_numerator == IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }

    let mut quotient_numerator = &dividend_numerator * &divisor_denominator;
    let mut quotient_denominator = &dividend_denominator * &divisor_numerator;
    if quotient_denominator < IBig::from(0) {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = &quotient_numerator / &quotient_denominator;
    let quotient = adjust_big_quotient(truncated, quotient_numerator, quotient_denominator, mode);
    let remainder = exact_ratio_value(
        &dividend_numerator * &divisor_denominator
            - &quotient * &divisor_numerator * &dividend_denominator,
        &dividend_denominator * &divisor_denominator,
    )?;
    Ok(Value::values(vec![Value::big_integer(quotient), remainder]))
}

fn exact_parts(value: &Number) -> Option<(IBig, IBig)> {
    match value {
        Number::Integer(value) => Some((IBig::from(*value), IBig::from(1))),
        Number::Big(value) => Some((value.clone(), IBig::from(1))),
        Number::Rational(value) => Some((value.numerator().clone(), value.denominator().clone())),
        Number::BigRational(value) => {
            Some((value.numerator().clone(), value.denominator().clone()))
        }
        Number::Float(_) => None,
    }
}

fn exact_ratio_value(mut numerator: IBig, mut denominator: IBig) -> Result<Value, RuntimeError> {
    if denominator == IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    if denominator < IBig::from(0) {
        numerator = -numerator;
        denominator = -denominator;
    }
    let divisor = numerator.gcd(&denominator);
    numerator /= &divisor;
    denominator /= &divisor;
    if denominator == IBig::from(1) {
        return Ok(Value::big_integer(numerator));
    }
    match (i128::try_from(&numerator), i128::try_from(&denominator)) {
        (Ok(numerator), Ok(denominator)) => Value::rational(numerator, denominator),
        _ => Value::big_rational(numerator, denominator),
    }
}

fn adjust_big_quotient(
    truncated: IBig,
    numerator: IBig,
    denominator: IBig,
    mode: RoundingMode,
) -> IBig {
    let remainder = &numerator % &denominator;
    if remainder == IBig::from(0) {
        return truncated;
    }
    let direction = if numerator < IBig::from(0) {
        -IBig::from(1)
    } else {
        IBig::from(1)
    };
    match mode {
        RoundingMode::Floor if direction < IBig::from(0) => truncated - 1,
        RoundingMode::Ceiling if direction > IBig::from(0) => truncated + 1,
        RoundingMode::Round => {
            let distance = ibig::ops::Abs::abs(&remainder) * 2;
            if distance > denominator
                || (distance == denominator && truncated.clone() % IBig::from(2) != IBig::from(0))
            {
                truncated + direction
            } else {
                truncated
            }
        }
        _ => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::float_quotient::{float_integer, round_float};
    use super::*;

    #[test]
    #[expect(clippy::float_cmp)]
    fn rounds_exact_and_float_quotients_by_mode() {
        assert_eq!(round_float(2.5), 2.0);
        assert_eq!(round_float(3.5), 4.0);
        assert!(float_integer(f64::INFINITY).is_err());
        let exact_zero_divisor = exact_quotient_and_remainder(
            &Number::Integer(1),
            &Number::Integer(0),
            RoundingMode::Floor,
        );
        assert!(matches!(
            exact_zero_divisor,
            Err(RuntimeError::DivisionByZero)
        ));
        let float_zero_divisor = float_quotient_and_remainder(
            &Number::Float(1.0),
            &Number::Float(0.0),
            RoundingMode::Floor,
        );
        assert!(matches!(
            float_zero_divisor,
            Err(RuntimeError::DivisionByZero)
        ));
    }

    #[test]
    fn exact_quotient_rejects_float_dividend_or_divisor() {
        assert!(matches!(
            exact_quotient_and_remainder(
                &Number::Float(1.0),
                &Number::Integer(1),
                RoundingMode::Floor
            ),
            Err(RuntimeError::InvalidForm { .. })
        ));
        assert!(matches!(
            exact_quotient_and_remainder(
                &Number::Integer(1),
                &Number::Float(1.0),
                RoundingMode::Floor
            ),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }

    #[test]
    fn exact_quotient_supports_bignum_dividends_and_quotients() {
        let dividend = Number::Big((IBig::from(1) << 80) + 1);
        let result =
            exact_quotient_and_remainder(&dividend, &Number::Integer(2), RoundingMode::Floor);
        assert!(matches!(
            result,
            Ok(Value::Values(ref values))
                if values.len() == 2
                    && values[0].to_string() == "604462909807314587353088"
                    && matches!(values[1], Value::Integer(1))
        ));
    }

    #[test]
    fn exact_quotient_normalizes_a_negative_quotient_denominator() {
        let result = exact_quotient_and_remainder(
            &Number::Integer(1),
            &Number::Integer(-2),
            RoundingMode::Truncate,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn float_quotient_supports_ceiling_and_truncate_modes() {
        let ceiling_result = float_quotient_and_remainder(
            &Number::Float(5.0),
            &Number::Float(2.0),
            RoundingMode::Ceiling,
        );
        assert!(matches!(
            ceiling_result,
            Ok(Value::Values(ref values)) if matches!(values[0], Value::Integer(3))
        ));

        let truncate_result = float_quotient_and_remainder(
            &Number::Float(-5.0),
            &Number::Float(2.0),
            RoundingMode::Truncate,
        );
        assert!(matches!(
            truncate_result,
            Ok(Value::Values(ref values)) if matches!(values[0], Value::Integer(-2))
        ));
    }

    #[test]
    fn float_quotient_overflows_when_the_rounded_ratio_exceeds_i64_range() {
        let result = float_quotient_and_remainder(
            &Number::Float(f64::MAX),
            &Number::Float(1.0),
            RoundingMode::Floor,
        );
        assert!(matches!(result, Err(RuntimeError::NumericOverflow)));
    }
}

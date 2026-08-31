use crate::error::RuntimeError;
use ibig::IBig;

#[derive(Clone, Debug, Eq, PartialEq)]
/// An exact, normalized rational number.
pub struct Rational {
    numerator: IBig,
    denominator: IBig,
}

impl Rational {
    pub(crate) fn new(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        if denominator == 0 {
            return Err(RuntimeError::DivisionByZero);
        }

        let (numerator, denominator) = if denominator < 0 {
            (
                numerator
                    .checked_neg()
                    .ok_or(RuntimeError::NumericOverflow)?,
                denominator
                    .checked_neg()
                    .ok_or(RuntimeError::NumericOverflow)?,
            )
        } else {
            (numerator, denominator)
        };

        let numerator_abs = if numerator < 0 {
            u128::try_from(
                numerator
                    .checked_neg()
                    .ok_or(RuntimeError::NumericOverflow)?,
            )
            .map_err(|_| RuntimeError::NumericOverflow)?
        } else {
            u128::try_from(numerator).map_err(|_| RuntimeError::NumericOverflow)?
        };
        let denominator_abs =
            u128::try_from(denominator).map_err(|_| RuntimeError::NumericOverflow)?;
        let divisor = gcd(numerator_abs, denominator_abs);
        let divisor = i128::try_from(divisor).map_err(|_| RuntimeError::NumericOverflow)?;
        Ok(Self {
            numerator: IBig::from(numerator / divisor),
            denominator: IBig::from(denominator / divisor),
        })
    }

    pub(crate) fn new_big(numerator: IBig, denominator: IBig) -> Result<Self, RuntimeError> {
        if denominator == IBig::from(0) {
            return Err(RuntimeError::DivisionByZero);
        }
        let (numerator, denominator) = if denominator < IBig::from(0) {
            (-numerator, -denominator)
        } else {
            (numerator, denominator)
        };
        let divisor = numerator.gcd(&denominator);
        Ok(Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        })
    }

    pub(crate) const fn numerator(&self) -> &IBig {
        &self.numerator
    }

    pub(crate) const fn denominator(&self) -> &IBig {
        &self.denominator
    }

    pub(crate) fn numerator_i64(&self) -> Result<i64, RuntimeError> {
        i64::try_from(&self.numerator).map_err(|_| RuntimeError::NumericOverflow)
    }

    pub(crate) fn denominator_i64(&self) -> Result<i64, RuntimeError> {
        i64::try_from(&self.denominator).map_err(|_| RuntimeError::NumericOverflow)
    }
}

const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use ibig::IBig;

    use super::Rational;
    use crate::error::RuntimeError;

    #[test]
    fn normalizes_sign_and_common_factor() {
        let negative_negative = match Rational::new(-6, -8) {
            Ok(value) => value,
            Err(error) => panic!("unexpected normalization error: {error:?}"),
        };
        let positive_denominator = match Rational::new(3, 4) {
            Ok(value) => value,
            Err(error) => panic!("unexpected normalization error: {error:?}"),
        };
        assert_eq!(negative_negative, positive_denominator);

        let negative_denominator = match Rational::new(6, -8) {
            Ok(value) => value,
            Err(error) => panic!("unexpected normalization error: {error:?}"),
        };
        let negative_numerator = match Rational::new(-3, 4) {
            Ok(value) => value,
            Err(error) => panic!("unexpected normalization error: {error:?}"),
        };
        assert_eq!(negative_denominator, negative_numerator);
    }

    #[test]
    fn preserves_zero_with_a_positive_denominator() {
        let value = match Rational::new(0, -9) {
            Ok(value) => value,
            Err(error) => panic!("unexpected normalization error: {error:?}"),
        };
        assert_eq!(value.numerator(), &IBig::from(0));
        assert_eq!(value.denominator(), &IBig::from(1));
    }

    #[test]
    fn rejects_zero_denominator_and_unrepresentable_values() {
        assert_eq!(Rational::new(1, 0), Err(RuntimeError::DivisionByZero));
        assert_eq!(
            Rational::new(i128::MIN, 1),
            Err(RuntimeError::NumericOverflow)
        );
        assert_eq!(
            Rational::new(1, i128::MIN),
            Err(RuntimeError::NumericOverflow)
        );
    }

    #[test]
    fn rejects_unrepresentable_i128_inputs_and_preserves_large_denominators() {
        // A negative denominator negates the numerator first; i128::MIN has no
        // positive counterpart, so that negation must fail before normalization
        // proceeds any further.
        assert_eq!(
            Rational::new(i128::MIN, -1),
            Err(RuntimeError::NumericOverflow)
        );
        let value = match Rational::new(1, i128::from(i64::MAX) + 1) {
            Ok(value) => value,
            Err(error) => panic!("unexpected rational construction error: {error:?}"),
        };
        assert_eq!(value.numerator(), &IBig::from(1));
        let expected_denominator = IBig::from(i64::MAX) + 1;
        assert_eq!(value.denominator(), &expected_denominator);
    }
}

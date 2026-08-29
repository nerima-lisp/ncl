use crate::error::RuntimeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// An exact, normalized rational number.
pub struct Rational {
    numerator: i64,
    denominator: i64,
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
        let numerator =
            i64::try_from(numerator / divisor).map_err(|_| RuntimeError::NumericOverflow)?;
        let denominator =
            i64::try_from(denominator / divisor).map_err(|_| RuntimeError::NumericOverflow)?;

        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub(crate) const fn numerator(self) -> i64 {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> i64 {
        self.denominator
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
        assert_eq!(value.numerator(), 0);
        assert_eq!(value.denominator(), 1);
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
    fn rejects_unrepresentable_numerators_and_denominators_after_sign_normalization() {
        // A negative denominator negates the numerator first; i128::MIN has no
        // positive counterpart, so that negation must fail before normalization
        // proceeds any further.
        assert_eq!(
            Rational::new(i128::MIN, -1),
            Err(RuntimeError::NumericOverflow)
        );
        // The reduced denominator can still overflow i64 even when the
        // reduced numerator fits, since gcd-reduction is independent per side.
        assert_eq!(
            Rational::new(1, i128::from(i64::MAX) + 1),
            Err(RuntimeError::NumericOverflow)
        );
    }
}

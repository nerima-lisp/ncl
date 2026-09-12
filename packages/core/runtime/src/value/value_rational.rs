use crate::error::RuntimeError;

#[derive(Clone, Debug, Eq, PartialEq)]
/// An exact, normalized rational number.
pub struct Rational {
    numerator: ibig::IBig,
    denominator: ibig::IBig,
}

impl Rational {
    pub(crate) fn new(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        Self::from_big(ibig::IBig::from(numerator), ibig::IBig::from(denominator))
    }

    pub(crate) fn from_big(
        mut numerator: ibig::IBig,
        mut denominator: ibig::IBig,
    ) -> Result<Self, RuntimeError> {
        if denominator == ibig::IBig::from(0) {
            return Err(RuntimeError::DivisionByZero);
        }

        if denominator < ibig::IBig::from(0) {
            numerator = -numerator;
            denominator = -denominator;
        }

        let numerator_abs = if numerator < ibig::IBig::from(0) {
            -&numerator
        } else {
            numerator.clone()
        };
        let denominator_abs = if denominator < ibig::IBig::from(0) {
            -&denominator
        } else {
            denominator.clone()
        };
        let divisor = numerator_abs.gcd(&denominator_abs);
        let numerator = numerator / &divisor;
        let denominator = denominator / &divisor;

        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub(crate) const fn numerator(&self) -> &ibig::IBig {
        &self.numerator
    }

    pub(crate) const fn denominator(&self) -> &ibig::IBig {
        &self.denominator
    }

    pub(crate) fn numerator_i128(&self) -> Option<i128> {
        self.numerator.to_string().parse().ok()
    }

    pub(crate) fn denominator_i128(&self) -> Option<i128> {
        self.denominator.to_string().parse().ok()
    }

    pub(crate) fn numerator_f64(&self) -> f64 {
        self.numerator.to_string().parse().unwrap_or_else(|_| {
            if self.numerator < ibig::IBig::from(0) {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        })
    }

    pub(crate) fn denominator_f64(&self) -> f64 {
        self.denominator
            .to_string()
            .parse()
            .unwrap_or(f64::INFINITY)
    }
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
        assert_eq!(value.numerator(), &ibig::IBig::from(0));
        assert_eq!(value.denominator(), &ibig::IBig::from(1));
    }

    #[test]
    fn rejects_zero_denominator() {
        assert_eq!(Rational::new(1, 0), Err(RuntimeError::DivisionByZero));
    }

    #[test]
    fn supports_large_numerators_and_denominators() {
        let value = match Rational::from_big(ibig::IBig::from(1) << 200, ibig::IBig::from(3)) {
            Ok(value) => value,
            Err(error) => panic!("unexpected large rational error: {error:?}"),
        };
        assert_eq!(value.denominator(), &ibig::IBig::from(3));
    }
}

use ibig::IBig;

use crate::error::RuntimeError;

#[derive(Clone, Debug, Eq, PartialEq)]
/// An exact rational whose numerator and denominator are arbitrary-precision integers.
pub struct BigRational {
    numerator: IBig,
    denominator: IBig,
}

impl BigRational {
    pub(crate) fn new(mut numerator: IBig, mut denominator: IBig) -> Result<Self, RuntimeError> {
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
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub(crate) fn numerator(&self) -> &IBig {
        &self.numerator
    }

    pub(crate) fn denominator(&self) -> &IBig {
        &self.denominator
    }
}

use std::rc::Rc;

use super::Value;

#[derive(Clone, Debug)]
pub struct Complex {
    real: Value,
    imaginary: Value,
}

impl Complex {
    pub(crate) fn new(real: Value, imaginary: Value) -> Self {
        Self { real, imaginary }
    }

    pub(crate) fn real(&self) -> &Value {
        &self.real
    }

    pub(crate) fn imaginary(&self) -> &Value {
        &self.imaginary
    }
}

impl Value {
    pub(crate) fn is_real_number(&self) -> bool {
        matches!(
            self,
            Self::Integer(_)
                | Self::BigInteger(_)
                | Self::Rational(_)
                | Self::BigRational(_)
                | Self::Float(_)
        )
    }

    pub(crate) fn complex(real: Self, imaginary: Self) -> Self {
        debug_assert!(real.is_real_number() && imaginary.is_real_number());
        let imaginary_is_zero = match &imaginary {
            Self::Integer(value) => *value == 0,
            Self::BigInteger(value) => value.as_ref() == &ibig::IBig::from(0),
            Self::Rational(value) => value.numerator() == &ibig::IBig::from(0),
            Self::BigRational(value) => value.numerator() == &ibig::IBig::from(0),
            Self::Float(_) => false,
            _ => false,
        };
        if imaginary_is_zero {
            return real;
        }
        Self::Complex(Rc::new(Complex::new(real, imaginary)))
    }
}

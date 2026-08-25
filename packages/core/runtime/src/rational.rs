use crate::error::RuntimeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            numerator
                .checked_neg()
                .ok_or(RuntimeError::NumericOverflow)? as u128
        } else {
            numerator as u128
        };
        let denominator_abs = denominator as u128;
        let divisor = gcd(numerator_abs, denominator_abs);
        let numerator = i64::try_from(numerator / divisor as i128)
            .map_err(|_| RuntimeError::NumericOverflow)?;
        let denominator = i64::try_from(denominator / divisor as i128)
            .map_err(|_| RuntimeError::NumericOverflow)?;

        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub(crate) fn numerator(self) -> i64 {
        self.numerator
    }

    pub(crate) fn denominator(self) -> i64 {
        self.denominator
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

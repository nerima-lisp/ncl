//! Signed integer arithmetic with i128 fast paths and limb-backed overflow.

use core::cmp::Ordering;

use super::big_integer::BigInteger;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Integer {
    Small(i128),
    Big(BigInteger),
}

impl Integer {
    pub(crate) const ZERO: Self = Self::Small(0);

    pub(crate) const fn from_i128(value: i128) -> Self {
        Self::Small(value)
    }

    #[cfg(test)]
    pub(crate) fn from_limbs(negative: bool, limbs: &[u32]) -> Self {
        Self::from_big(BigInteger {
            negative,
            limbs: limbs.to_vec(),
        })
    }

    pub(crate) fn from_owned_limbs(negative: bool, limbs: Vec<u32>) -> Self {
        Self::from_big(BigInteger { negative, limbs })
    }

    fn from_big(mut value: BigInteger) -> Self {
        BigInteger::normalize(&mut value.limbs);
        if value.limbs.is_empty() {
            return Self::ZERO;
        }
        if let Some(value) = value.to_i128() {
            return Self::Small(value);
        }
        Self::Big(value)
    }

    pub(crate) fn to_i128(&self) -> Option<i128> {
        match self {
            Self::Small(value) => Some(*value),
            Self::Big(value) => value.to_i128(),
        }
    }

    pub(crate) const fn is_zero(&self) -> bool {
        match self {
            Self::Small(value) => *value == 0,
            Self::Big(value) => value.limbs.is_empty(),
        }
    }

    pub(crate) const fn is_negative(&self) -> bool {
        match self {
            Self::Small(value) => *value < 0,
            Self::Big(value) => value.negative,
        }
    }

    pub(crate) fn is_even(&self) -> bool {
        match self {
            Self::Small(value) => value & 1 == 0,
            Self::Big(value) => value.limbs.first().is_none_or(|limb| limb & 1 == 0),
        }
    }

    pub(crate) fn signum(&self) -> Self {
        match self.cmp(&Self::ZERO) {
            Ordering::Less => Self::Small(-1),
            Ordering::Equal => Self::ZERO,
            Ordering::Greater => Self::Small(1),
        }
    }

    pub(crate) fn abs(&self) -> Self {
        match self {
            Self::Small(value) => value.checked_abs().map_or_else(
                || Self::from_big(BigInteger::from_i128(*value).with_sign(false)),
                Self::Small,
            ),
            Self::Big(value) => Self::from_big(value.clone().with_sign(false)),
        }
    }

    pub(crate) fn negate(&self) -> Self {
        match self {
            Self::Small(value) => value.checked_neg().map_or_else(
                || Self::from_big(BigInteger::from_i128(*value).with_sign(false)),
                Self::Small,
            ),
            Self::Big(value) => Self::from_big(value.clone().with_sign(!value.negative)),
        }
    }

    pub(crate) fn add(&self, other: &Self) -> Option<Self> {
        if let (Self::Small(left), Self::Small(right)) = (self, other)
            && let Some(value) = left.checked_add(*right)
        {
            return Some(Self::Small(value));
        }
        Some(Self::from_big(self.to_big().add(&other.to_big())?))
    }

    pub(crate) fn sub(&self, other: &Self) -> Option<Self> {
        if let (Self::Small(left), Self::Small(right)) = (self, other)
            && let Some(value) = left.checked_sub(*right)
        {
            return Some(Self::Small(value));
        }
        Some(Self::from_big(self.to_big().sub(&other.to_big())?))
    }

    pub(crate) fn mul(&self, other: &Self) -> Option<Self> {
        if let (Self::Small(left), Self::Small(right)) = (self, other)
            && let Some(value) = left.checked_mul(*right)
        {
            return Some(Self::Small(value));
        }
        Some(Self::from_big(self.to_big().mul(&other.to_big())?))
    }

    pub(crate) fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Small(left), Self::Small(right)) => left.cmp(right),
            (Self::Big(left), Self::Big(right)) => left.cmp(right),
            (Self::Big(left), Self::Small(right)) => left.cmp_i128(*right),
            (Self::Small(left), Self::Big(right)) => right.cmp_i128(*left).reverse(),
        }
    }

    pub(crate) fn div_trunc_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        if divisor.is_zero() {
            return None;
        }
        if let (Self::Small(left), Self::Small(right)) = (self, divisor)
            && let (Some(quotient), Some(remainder)) =
                (left.checked_div(*right), left.checked_rem(*right))
        {
            return Some((Self::Small(quotient), Self::Small(remainder)));
        }
        let numerator = self.to_big();
        let denominator = divisor.to_big();
        let (quotient, remainder) = BigInteger::div_rem_abs(&numerator.limbs, &denominator.limbs)?;
        Some((
            Self::from_big(BigInteger {
                negative: numerator.negative != denominator.negative,
                limbs: quotient,
            }),
            Self::from_big(BigInteger {
                negative: numerator.negative,
                limbs: remainder,
            }),
        ))
    }

    pub(crate) fn div_floor_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        let (quotient, remainder) = self.div_trunc_rem(divisor)?;
        if !remainder.is_zero() && self.is_negative() != divisor.is_negative() {
            Some((quotient.sub(&Self::Small(1))?, remainder.add(divisor)?))
        } else {
            Some((quotient, remainder))
        }
    }

    pub(crate) fn div_ceil_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        let (quotient, remainder) = self.div_trunc_rem(divisor)?;
        if !remainder.is_zero() && self.is_negative() == divisor.is_negative() {
            Some((quotient.add(&Self::Small(1))?, remainder.sub(divisor)?))
        } else {
            Some((quotient, remainder))
        }
    }

    pub(crate) fn ash(&self, shift: i128) -> Option<Self> {
        if shift < 0 {
            let magnitude = shift.unsigned_abs();
            if let Self::Small(value) = self
                && magnitude >= 128
            {
                return Some(Self::Small(if *value < 0 { -1 } else { 0 }));
            }
            let Ok(shift) = usize::try_from(magnitude) else {
                return Some(if self.is_negative() {
                    Self::Small(-1)
                } else {
                    Self::ZERO
                });
            };
            if let Self::Small(value) = self {
                let shift = u32::try_from(shift).ok()?;
                return Some(Self::Small(if shift >= 128 {
                    if *value < 0 { -1 } else { 0 }
                } else {
                    *value >> shift
                }));
            }
            return Some(Self::from_big(self.to_big().shift_right_arithmetic(shift)?));
        }

        if let Self::Small(value) = self
            && let Ok(shift) = u32::try_from(shift)
            && let Some(result) = value.checked_shl(shift)
            && (shift == 0 || result >> shift == *value)
        {
            return Some(Self::Small(result));
        }
        let shift = usize::try_from(shift).ok()?;
        Some(Self::from_big(self.to_big().shift_left(shift)?))
    }

    pub(crate) fn ash_by(&self, shift: &Self) -> Option<Self> {
        if let Some(shift) = shift.to_i128() {
            return self.ash(shift);
        }
        if shift.is_negative() {
            Some(if self.is_negative() {
                Self::Small(-1)
            } else {
                Self::ZERO
            })
        } else {
            None
        }
    }

    pub(crate) fn to_f64(&self) -> f64 {
        let (negative, limbs) = match self {
            Self::Small(value) => {
                let bytes = value.unsigned_abs().to_le_bytes();
                let mut magnitude = 0.0_f64;
                for limb_bytes in bytes.as_chunks::<4>().0.iter().rev() {
                    magnitude = magnitude
                        .mul_add(4_294_967_296.0, f64::from(u32::from_le_bytes(*limb_bytes)));
                }
                return if *value < 0 { -magnitude } else { magnitude };
            }
            Self::Big(value) => (value.negative, value.limbs.as_slice()),
        };
        let mut magnitude = 0.0_f64;
        for limb in limbs.iter().rev() {
            magnitude = magnitude.mul_add(4_294_967_296.0, f64::from(*limb));
        }
        if negative { -magnitude } else { magnitude }
    }

    pub(crate) fn into_limbs_and_sign(self) -> (bool, Vec<u32>) {
        match self {
            Self::Small(value) => {
                let big = BigInteger::from_i128(value);
                (big.negative, big.limbs)
            }
            Self::Big(value) => (value.negative, value.limbs),
        }
    }

    fn to_big(&self) -> BigInteger {
        match self {
            Self::Small(value) => BigInteger::from_i128(*value),
            Self::Big(value) => value.clone(),
        }
    }
}

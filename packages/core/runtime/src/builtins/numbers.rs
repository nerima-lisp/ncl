#![allow(clippy::wildcard_imports)]
use super::*;
use crate::BigRational;

mod conversions;
pub(super) use conversions::{
    big_rational_number, exceeds_exact_bignum_digit_cap, integer_argument, integer_value, number,
    number_argument, number_from_big, number_to_value, rational_number,
};

mod arithmetic;
pub(super) use arithmetic::{exact_binary, negate_number};

mod comparison;
pub(super) use comparison::{compare_number_values, numeric_equalp};

#[derive(Clone)]
pub(super) enum Number {
    Integer(i64),
    /// Never holds a value that fits in `i64` -- every construction site
    /// routes through [`Value::big_integer`] or [`number_from_big`], both
    /// of which demote such a value to [`Number::Integer`] first. Match
    /// arms may therefore rely on this being large in magnitude, and in
    /// particular on it never being zero: `signum`'s two-way sign test
    /// would otherwise report `1` for a bignum-typed zero.
    Big(ibig::IBig),
    Rational(Rational),
    BigRational(BigRational),
    Float(f64),
}

impl Number {
    #[expect(
        clippy::cast_precision_loss,
        reason = "Common Lisp coercion to single precision semantics uses f64"
    )]
    pub(crate) fn as_float(&self) -> f64 {
        match self {
            Self::Integer(value) => *value as f64,
            Self::Big(value) => value.to_string().parse().unwrap_or(f64::INFINITY),
            Self::Rational(value) => value.numerator_f64() / value.denominator_f64(),
            Self::BigRational(value) => {
                let numerator = value.numerator().to_string().parse::<f64>().unwrap_or(
                    if value.numerator() < &ibig::IBig::from(0) {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    },
                );
                let denominator = value
                    .denominator()
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(f64::INFINITY);
                numerator / denominator
            }
            Self::Float(value) => *value,
        }
    }

    pub(super) const fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub(super) fn exact_parts(&self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((*value, 1)),
            Self::Rational(value) => Some((
                value.numerator_i128()?.try_into().ok()?,
                value.denominator_i128()?.try_into().ok()?,
            )),
            Self::Big(_) | Self::BigRational(_) | Self::Float(_) => None,
        }
    }

    pub(crate) fn exact_big_parts(&self) -> Option<(ibig::IBig, ibig::IBig)> {
        match self {
            Self::Integer(value) => Some((ibig::IBig::from(*value), ibig::IBig::from(1))),
            Self::Big(value) => Some((value.clone(), ibig::IBig::from(1))),
            Self::Rational(value) => Some((value.numerator().clone(), value.denominator().clone())),
            Self::BigRational(value) => {
                Some((value.numerator().clone(), value.denominator().clone()))
            }
            Self::Float(_) => None,
        }
    }
}

impl Value {
    pub(super) const fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

pub(crate) fn big_integer_argument(
    function: &str,
    value: &Value,
) -> Result<ibig::IBig, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(ibig::IBig::from(*value)),
        Value::BigInteger(value) => Ok(value.as_ref().clone()),
        value => Err(crate::builtins::builtin_helpers::type_error(
            function, "integer", value,
        )),
    }
}

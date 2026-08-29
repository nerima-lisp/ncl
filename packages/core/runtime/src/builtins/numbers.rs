#![allow(clippy::wildcard_imports)]
use super::*;

mod conversions;
pub(super) use conversions::{
    integer_argument, number, number_argument, number_from_big, number_to_value, rational_number,
};

mod arithmetic;
pub(super) use arithmetic::{exact_binary, negate_number};

mod comparison;
pub(super) use comparison::{compare_number_values, numeric_equalp};

#[derive(Clone)]
pub(super) enum Number {
    Integer(i64),
    Big(ibig::IBig),
    Rational(Rational),
    Float(f64),
}

impl Number {
    #[expect(
        clippy::cast_precision_loss,
        reason = "Common Lisp coercion to single precision semantics uses f64"
    )]
    pub(super) fn as_float(&self) -> f64 {
        match self {
            Self::Integer(value) => *value as f64,
            Self::Big(value) => value.to_string().parse().unwrap_or(f64::INFINITY),
            Self::Rational(value) => value.numerator() as f64 / value.denominator() as f64,
            Self::Float(value) => *value,
        }
    }

    pub(super) const fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub(super) const fn exact_parts(&self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((*value, 1)),
            Self::Rational(value) => Some((value.numerator(), value.denominator())),
            Self::Big(_) | Self::Float(_) => None,
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

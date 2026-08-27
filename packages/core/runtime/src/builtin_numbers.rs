#![allow(clippy::wildcard_imports)]
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Number {
    Integer(i64),
    Rational(Rational),
    Float(f64),
}

impl Number {
    #[expect(
        clippy::cast_precision_loss,
        reason = "Common Lisp coercion to single precision semantics uses f64"
    )]
    pub(super) fn as_float(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Rational(value) => value.numerator() as f64 / value.denominator() as f64,
            Self::Float(value) => value,
        }
    }

    pub(super) const fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub(super) const fn exact_parts(self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((value, 1)),
            Self::Rational(value) => Some((value.numerator(), value.denominator())),
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

pub(super) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(super) fn number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

pub(super) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

pub(super) fn rational_number(numerator: i128, denominator: i128) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
    }
}

pub(super) fn exact_binary(
    left: Number,
    right: Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let Some((left_numerator, left_denominator)) = left.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    let Some((right_numerator, right_denominator)) = right.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    let left_numerator = i128::from(left_numerator);
    let left_denominator = i128::from(left_denominator);
    let right_numerator = i128::from(right_numerator);
    let right_denominator = i128::from(right_denominator);
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * right_denominator + right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * right_denominator - right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => (
            left_numerator * right_denominator,
            left_denominator * right_numerator,
        ),
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: "unsupported exact numeric operation".to_string(),
                span: None,
            });
        }
    };
    rational_number(numerator, denominator)
}

pub(super) fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => value
            .checked_neg()
            .map(Number::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => rational_number(
            -i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}

pub(super) fn compare_number_values(left: Number, right: Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let Some((left_numerator, left_denominator)) = left.exact_parts() else {
        return Ordering::Equal;
    };
    let Some((right_numerator, right_denominator)) = right.exact_parts() else {
        return Ordering::Equal;
    };
    (i128::from(left_numerator) * i128::from(right_denominator))
        .cmp(&(i128::from(right_numerator) * i128::from(left_denominator)))
}

pub(super) fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

pub(super) fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

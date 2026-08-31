use std::cmp::Ordering;

use ibig::IBig;

use super::Number;

/// Converts any non-float exact `Number` into an `(numerator, denominator)`
/// pair over arbitrary-precision integers, so a bignum can be compared
/// exactly against a plain integer or a rational rather than only against
/// another bignum.
fn as_big_ratio(value: &Number) -> Option<(IBig, IBig)> {
    match value {
        Number::Integer(value) => Some((IBig::from(*value), IBig::from(1))),
        Number::Big(value) => Some((value.clone(), IBig::from(1))),
        Number::Rational(value) => Some((value.numerator().clone(), value.denominator().clone())),
        Number::Float(_) => None,
    }
}

pub(in crate::builtins) fn compare_number_values(left: &Number, right: &Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let (Some((left_numerator, left_denominator)), Some((right_numerator, right_denominator))) =
        (as_big_ratio(left), as_big_ratio(right))
    else {
        unreachable!("as_big_ratio only rejects Float, excluded above");
    };
    (left_numerator * right_denominator).cmp(&(right_numerator * left_denominator))
}

pub(in crate::builtins) fn numeric_equalp(left: &Number, right: &Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

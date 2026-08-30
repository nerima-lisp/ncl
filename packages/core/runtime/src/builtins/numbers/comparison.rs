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
        Number::Rational(value) => Some((
            IBig::from(value.numerator()),
            IBig::from(value.denominator()),
        )),
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
    if let (Number::Big(left), Number::Big(right)) = (left, right) {
        // Both operands are already bare integers: compare by reference
        // directly rather than padding each into a ratio and cloning.
        return left.cmp(right);
    }
    if matches!(left, Number::Big(_)) || matches!(right, Number::Big(_)) {
        let (Some((left_numerator, left_denominator)), Some((right_numerator, right_denominator))) =
            (as_big_ratio(left), as_big_ratio(right))
        else {
            // The is_float() check above already handled the only case
            // (a Float operand) that as_big_ratio cannot convert.
            return Ordering::Equal;
        };
        return (left_numerator * right_denominator).cmp(&(right_numerator * left_denominator));
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

pub(in crate::builtins) fn numeric_equalp(left: &Number, right: &Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

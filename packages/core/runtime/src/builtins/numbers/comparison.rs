use std::cmp::Ordering;

use super::Number;

pub(in crate::builtins) fn compare_number_values(left: Number, right: Number) -> Ordering {
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

pub(in crate::builtins) fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

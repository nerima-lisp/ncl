use std::cmp::Ordering;

use ncl_object::{
    Bignum, ObjectRef, ThreadContext, Word, bignum_limbs, bignum_sign, classify_object,
};

use crate::{IntegerBound, TypeError};

pub(super) fn typep_integer_range(
    ctx: &mut ThreadContext,
    object: Word,
    low: IntegerBound,
    high: IntegerBound,
) -> Result<bool, TypeError> {
    if let Some(value) = object.as_fixnum() {
        return Ok(bound_contains(low, value, true) && bound_contains(high, value, false));
    }
    let ObjectRef::Bignum(word) = classify_object(ctx, object) else {
        return Ok(false);
    };
    let bignum = Bignum::from_word(word);
    let negative = bignum_sign(ctx, bignum)?;
    let limbs = bignum_limbs(ctx, bignum)?;
    Ok(bound_contains_bignum(low, negative, &limbs, true)
        && bound_contains_bignum(high, negative, &limbs, false))
}

const fn bound_contains(bound: IntegerBound, value: i64, lower: bool) -> bool {
    match bound {
        IntegerBound::Unbounded => true,
        IntegerBound::Inclusive(bound) => {
            if lower {
                value >= bound
            } else {
                value <= bound
            }
        }
        IntegerBound::Exclusive(bound) => {
            if lower {
                value > bound
            } else {
                value < bound
            }
        }
    }
}

fn bound_contains_bignum(bound: IntegerBound, negative: bool, limbs: &[u32], lower: bool) -> bool {
    match bound {
        IntegerBound::Unbounded => true,
        IntegerBound::Inclusive(bound) => {
            let ordering = compare_bignum_to_i64(negative, limbs, bound);
            if lower {
                ordering != Ordering::Less
            } else {
                ordering != Ordering::Greater
            }
        }
        IntegerBound::Exclusive(bound) => {
            let ordering = compare_bignum_to_i64(negative, limbs, bound);
            if lower {
                ordering == Ordering::Greater
            } else {
                ordering == Ordering::Less
            }
        }
    }
}

fn compare_bignum_to_i64(negative: bool, limbs: &[u32], bound: i64) -> Ordering {
    let length = limbs
        .iter()
        .rposition(|limb| *limb != 0)
        .map_or(0, |index| index + 1);
    if length == 0 {
        return 0_i64.cmp(&bound);
    }

    if negative != bound.is_negative() {
        return if negative {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }

    let magnitude = bound.unsigned_abs();
    let bound_length = if magnitude >> 32 == 0 {
        usize::from(magnitude != 0)
    } else {
        2
    };
    let magnitude_ordering = match length.cmp(&bound_length) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => limbs
            .iter()
            .take(length)
            .enumerate()
            .fold(0_u64, |magnitude, (index, limb)| {
                magnitude | (u64::from(*limb) << (index * 32))
            })
            .cmp(&bound.unsigned_abs()),
    };
    if negative {
        magnitude_ordering.reverse()
    } else {
        magnitude_ordering
    }
}

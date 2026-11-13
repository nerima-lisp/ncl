use super::{HashTable, HashTest, Weakness};
use ncl_object::hash_table::{DEFAULT_CAPACITY, DEFAULT_REHASH_SIZE, DEFAULT_REHASH_THRESHOLD};
use ncl_object::{
    Bignum, DoubleFloat, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, Word, bignum_limbs,
    bignum_sign, classify_object, double_value, make_double, ratio_denominator, ratio_numerator,
};

/// Allocate a hash table from Common Lisp construction options.
///
/// # Errors
/// Returns an allocation or layout error, or a type error for invalid options.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn new_with_options(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    test: HashTest,
    weakness: Weakness,
    size: Word,
    rehash_size: Option<Word>,
    rehash_threshold: Option<Word>,
) -> Result<HashTable, ObjectError> {
    let capacity = normalize_capacity(positive_integer(ctx, size)?)?;
    if let Some(value) = rehash_size {
        validate_rehash_size(ctx, value)?;
    }
    if let Some(value) = rehash_threshold {
        validate_rehash_threshold(ctx, value)?;
    }
    let mut size = size;
    ncl_object::with_root(ctx, &mut size, |ctx, _size| {
        let mut rehash_size = match rehash_size {
            Some(value) => value,
            None => make_double(ctx, runtime, DEFAULT_REHASH_SIZE)?.as_word(),
        };
        ncl_object::with_root(ctx, &mut rehash_size, |ctx, rehash_size| {
            let mut rehash_threshold = match rehash_threshold {
                Some(value) => value,
                None => make_double(ctx, runtime, DEFAULT_REHASH_THRESHOLD)?.as_word(),
            };
            ncl_object::with_root(ctx, &mut rehash_threshold, |ctx, rehash_threshold| {
                HashTable::new_from_validated_options(
                    ctx,
                    runtime,
                    test,
                    weakness,
                    capacity,
                    *rehash_size,
                    *rehash_threshold,
                )
            })
        })
    })
}

fn positive_integer(ctx: &ThreadContext, word: Word) -> Result<u128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) if value > 0 => {
            u128::try_from(value).map_err(|_| ObjectError::TypeError)
        }
        ObjectRef::Bignum(value) => {
            let object = Bignum::from_word(value);
            if bignum_sign(ctx, object)? {
                return Err(ObjectError::TypeError);
            }
            let mut magnitude = 0_u128;
            for limb in bignum_limbs(ctx, object)?.into_iter().rev() {
                magnitude = magnitude
                    .checked_shl(32)
                    .and_then(|value| value.checked_add(u128::from(limb)))
                    .ok_or(ObjectError::TypeError)?;
            }
            if magnitude > 0 {
                Ok(magnitude)
            } else {
                Err(ObjectError::TypeError)
            }
        }
        // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive and unknown values are invalid.
        _ => Err(ObjectError::TypeError),
    }
}

fn normalize_capacity(size: u128) -> Result<usize, ObjectError> {
    let requested = usize::try_from(size).map_err(|_| ObjectError::TypeError)?;
    let requested = requested.max(DEFAULT_CAPACITY);
    let capacity = requested
        .checked_next_power_of_two()
        .ok_or(ObjectError::TypeError)?;
    if capacity > usize::MAX / 2 {
        Err(ObjectError::TypeError)
    } else {
        Ok(capacity)
    }
}

fn validate_rehash_size(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => validate_positive_integer(ctx, Word::fixnum(value)),
        ObjectRef::Bignum(value) => validate_positive_integer(ctx, value),
        ObjectRef::DoubleFloat(value) => {
            let value = double_value(ctx, DoubleFloat::from_word(value))?;
            if value.is_finite() && value > 1.0 {
                Ok(())
            } else {
                Err(ObjectError::TypeError)
            }
        }
        // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive and unknown values are invalid.
        _ => Err(ObjectError::TypeError),
    }
}

fn validate_positive_integer(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    if positive_integer(ctx, word)? > 0 {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

fn validate_rehash_threshold(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    let value = rehash_threshold_value(ctx, word)?;
    if value.is_finite() && value > 0.0 && value <= 1.0 {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

fn rehash_threshold_value(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(integer_to_f64(i128::from(value))),
        ObjectRef::Bignum(value) => {
            let object = Bignum::from_word(value);
            let magnitude = bignum_to_u128(ctx, object)?;
            let value = magnitude_to_f64(magnitude);
            Ok(if bignum_sign(ctx, object)? {
                -value
            } else {
                value
            })
        }
        ObjectRef::Ratio(value) => {
            let object = Ratio::from_word(value);
            let numerator = rehash_threshold_value(ctx, ratio_numerator(ctx, object)?)?;
            let denominator = rehash_threshold_value(ctx, ratio_denominator(ctx, object)?)?;
            Ok(numerator / denominator)
        }
        ObjectRef::DoubleFloat(value) => double_value(ctx, DoubleFloat::from_word(value)),
        // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive and unknown values are invalid.
        _ => Err(ObjectError::TypeError),
    }
}

fn bignum_to_u128(ctx: &ThreadContext, object: Bignum) -> Result<u128, ObjectError> {
    let mut magnitude = 0_u128;
    for limb in bignum_limbs(ctx, object)?.into_iter().rev() {
        magnitude = magnitude
            .checked_shl(32)
            .and_then(|value| value.checked_add(u128::from(limb)))
            .ok_or(ObjectError::TypeError)?;
    }
    Ok(magnitude)
}

fn magnitude_to_f64(mut magnitude: u128) -> f64 {
    let mut value = 0.0;
    let mut place = 1.0;
    while magnitude != 0 {
        if magnitude & 1 != 0 {
            value += place;
        }
        magnitude >>= 1;
        place *= 2.0;
    }
    value
}

fn integer_to_f64(value: i128) -> f64 {
    let magnitude = magnitude_to_f64(value.unsigned_abs());
    if value.is_negative() {
        -magnitude
    } else {
        magnitude
    }
}

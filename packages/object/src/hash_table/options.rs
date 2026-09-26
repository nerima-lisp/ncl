use super::{HashTable, HashTest, Weakness};
use crate::{
    Bignum, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, bignum_limbs, bignum_sign,
    classify_object, double_value, finish_root, make_double, ratio_denominator, ratio_numerator,
};
use ncl_sys::Word;

const DEFAULT_CAPACITY: usize = 8;
const DEFAULT_REHASH_SIZE: f64 = 1.5;
const DEFAULT_REHASH_THRESHOLD: f64 = 0.75;

#[derive(Clone, Copy, Debug)]
enum RehashSize {
    Add(usize),
    Multiply(f64),
}

impl HashTable {
    /// Allocate an empty heap hash table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
    ) -> Result<Self, ObjectError> {
        Self::new_with_options(
            ctx,
            runtime,
            test,
            weakness,
            Word::fixnum(i64::try_from(DEFAULT_CAPACITY).map_err(|_| ObjectError::Layout)?),
            None,
            None,
        )
    }

    /// Allocate an empty heap hash table with Common Lisp construction options.
    ///
    /// # Errors
    /// Returns an allocation or layout error, or a type error for invalid options.
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
    ) -> Result<Self, ObjectError> {
        let capacity = normalize_capacity(positive_integer(ctx, size)?)?;
        if let Some(value) = rehash_size {
            validate_rehash_size(ctx, value)?;
        }
        if let Some(value) = rehash_threshold {
            validate_rehash_threshold(ctx, value)?;
        }
        let mut size = size;
        let size_token = crate::push_root(ctx, &mut size);
        let mut rehash_size = match rehash_size {
            Some(value) => value,
            None => make_double(ctx, runtime, DEFAULT_REHASH_SIZE)?.as_word(),
        };
        let rehash_size_token = crate::push_root(ctx, &mut rehash_size);
        let mut rehash_threshold = match rehash_threshold {
            Some(value) => value,
            None => make_double(ctx, runtime, DEFAULT_REHASH_THRESHOLD)?.as_word(),
        };
        let rehash_threshold_token = crate::push_root(ctx, &mut rehash_threshold);
        let result = Self::allocate(
            ctx,
            runtime,
            test,
            weakness,
            capacity,
            rehash_size,
            rehash_threshold,
        );
        let result = finish_root(ctx, rehash_threshold_token, result);
        let result = finish_root(ctx, rehash_size_token, result);
        finish_root(ctx, size_token, result)
    }
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
            let value = double_value(ctx, crate::DoubleFloat::from_word(value))?;
            if value.is_finite() && value > 1.0 {
                Ok(())
            } else {
                Err(ObjectError::TypeError)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn validate_positive_integer(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    let value = positive_integer(ctx, word)?;
    if value > 0 {
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

fn rehash_size_value(ctx: &ThreadContext, word: Word) -> Result<RehashSize, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(RehashSize::Add(
            usize::try_from(positive_integer(ctx, Word::fixnum(value))?)
                .map_err(|_| ObjectError::TypeError)?,
        )),
        ObjectRef::Bignum(value) => Ok(RehashSize::Add(
            usize::try_from(positive_integer(ctx, value)?).map_err(|_| ObjectError::TypeError)?,
        )),
        ObjectRef::DoubleFloat(value) => {
            let value = double_value(ctx, crate::DoubleFloat::from_word(value))?;
            if value.is_finite() && value > 1.0 {
                Ok(RehashSize::Multiply(value))
            } else {
                Err(ObjectError::Layout)
            }
        }
        _ => Err(ObjectError::Layout),
    }
}

pub(super) fn rehash_threshold_value(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
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
        ObjectRef::DoubleFloat(value) => double_value(ctx, crate::DoubleFloat::from_word(value)),
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

pub(super) fn usize_to_f64(mut value: usize) -> f64 {
    let mut result = 0.0;
    let mut place = 1.0;
    while value != 0 {
        if value & 1 != 0 {
            result += place;
        }
        value >>= 1;
        place *= 2.0;
    }
    result
}

pub(super) fn next_capacity(ctx: &ThreadContext, table: HashTable) -> Result<usize, ObjectError> {
    let capacity = table.capacity(ctx)?;
    let requested = match rehash_size_value(ctx, table.rehash_size(ctx)?)? {
        RehashSize::Add(amount) => capacity.checked_add(amount).ok_or(ObjectError::Layout)?,
        RehashSize::Multiply(factor) => scaled_capacity(capacity, factor)?,
    };
    normalize_capacity(u128::try_from(requested).map_err(|_| ObjectError::Layout)?)
}

fn scaled_capacity(capacity: usize, factor: f64) -> Result<usize, ObjectError> {
    let (numerator, shift) = float_ratio(factor)?;
    let capacity = u128::try_from(capacity).map_err(|_| ObjectError::Layout)?;
    let product = capacity.checked_mul(numerator).ok_or(ObjectError::Layout)?;
    let denominator = 1_u128.checked_shl(shift).ok_or(ObjectError::Layout)?;
    let rounded = product
        .checked_add(denominator - 1)
        .ok_or(ObjectError::Layout)?
        / denominator;
    let minimum = capacity.checked_add(1).ok_or(ObjectError::Layout)?;
    usize::try_from(rounded.max(minimum)).map_err(|_| ObjectError::Layout)
}

fn float_ratio(value: f64) -> Result<(u128, u32), ObjectError> {
    let bits = value.to_bits();
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (u128::from(fraction), -1074_i32)
    } else {
        let exponent = i32::try_from(exponent_bits).map_err(|_| ObjectError::Layout)?;
        (u128::from((1_u64 << 52) | fraction), exponent - 1023 - 52)
    };
    if exponent >= 0 {
        let shift = u32::try_from(exponent).map_err(|_| ObjectError::Layout)?;
        Ok((
            significand.checked_shl(shift).ok_or(ObjectError::Layout)?,
            0,
        ))
    } else {
        Ok((significand, exponent.unsigned_abs()))
    }
}

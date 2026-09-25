//! Integer bit operations and byte manipulation.
//!
//! The callbacks in this module deliberately contain no symbol registration.  The
//! numbers registrar can install them alongside the other numeric callbacks.

use ncl_object::{
    Bignum, ObjectError, ObjectRef, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign,
    classify_object, make_bignum_from_i128,
};

use ncl_object::{BuiltinArgs, MultipleValues};

use super::*;

fn integer(ctx: &ThreadContext, value: Word) -> Result<i128, ObjectError> {
    if let Some(value) = value.as_fixnum() {
        return Ok(i128::from(value));
    }
    let ObjectRef::Bignum(word) = classify_object(ctx, value) else {
        return Err(ObjectError::TypeError);
    };
    let object = Bignum::from_word(word);
    let limbs = bignum_limbs(ctx, object)?;
    let mut magnitude = 0_i128;
    for (index, limb) in limbs.iter().enumerate() {
        let shift = index.checked_mul(32).ok_or(ObjectError::Layout)?;
        if shift >= 127 || (i128::from(*limb) << shift) < 0 {
            return Err(ObjectError::Layout);
        }
        magnitude |= i128::from(*limb) << shift;
    }
    let negative = bignum_sign(ctx, object)?;
    if negative {
        magnitude.checked_neg().ok_or(ObjectError::Layout)
    } else {
        Ok(magnitude)
    }
}

fn integer_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: i128,
) -> Result<Word, ObjectError> {
    if let Ok(value) = i64::try_from(value) {
        if let Some(bits) = value.checked_shl(1) {
            let word = Word::from_bits(bits as u64);
            if word.as_fixnum() == Some(value) {
                return Ok(word);
            }
        }
    }
    Ok(make_bignum_from_i128(ctx, runtime, value)?.into())
}

fn all_integers(ctx: &ThreadContext, args: &[Word]) -> Result<Vec<i128>, ObjectError> {
    args.iter().map(|&arg| integer(ctx, arg)).collect()
}

fn fold_bits<F>(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    init: i128,
    op: F,
) -> Result<Word, ObjectError>
where
    F: Fn(i128, i128) -> i128,
{
    let values = all_integers(ctx, args)?;
    let result = values.into_iter().fold(init, op);
    integer_word(ctx, runtime, result)
}

pub fn logand(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fold_bits(runtime, ctx, args, -1, |a, b| a & b)
}

pub fn logior(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fold_bits(runtime, ctx, args, 0, |a, b| a | b)
}

pub fn logxor(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fold_bits(runtime, ctx, args, 0, |a, b| a ^ b)
}

pub fn lognot(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [value] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, !integer(ctx, *value)?)
}

pub fn logeqv(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fold_bits(runtime, ctx, args, -1, |a, b| !(a ^ b))
}

pub fn lognand(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, !(integer(ctx, *a)? & integer(ctx, *b)?))
}

pub fn lognor(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, !(integer(ctx, *a)? | integer(ctx, *b)?))
}

pub fn logandc1(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, !integer(ctx, *a)? & integer(ctx, *b)?)
}

pub fn logandc2(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, integer(ctx, *a)? & !integer(ctx, *b)?)
}

pub fn logorc1(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, !integer(ctx, *a)? | integer(ctx, *b)?)
}

pub fn logorc2(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, integer(ctx, *a)? | !integer(ctx, *b)?)
}

pub fn logtest(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(if integer(ctx, *a)? & integer(ctx, *b)? != 0 {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub fn logbitp(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [index, value] = args else {
        return Err(ObjectError::TypeError);
    };
    let index = usize::try_from(integer(ctx, *index)?).map_err(|_| ObjectError::TypeError)?;
    let value = integer(ctx, *value)?;
    Ok(if index >= 127 {
        if value < 0 { Word::TRUE } else { Word::NIL }
    } else if (value & (1_i128 << index)) != 0 {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub fn logcount(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [value] = args else {
        return Err(ObjectError::TypeError);
    };
    let value = integer(ctx, *value)?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Word::fixnum(i64::from(count)))
}

pub fn integer_length(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [value] = args else {
        return Err(ObjectError::TypeError);
    };
    let value = integer(ctx, *value)?;
    let bits = if value >= 0 {
        value.leading_zeros()
    } else {
        (!value).leading_zeros()
    };
    Ok(Word::fixnum(i64::from(128 - bits)))
}

pub fn ash(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [value, count] = args else {
        return Err(ObjectError::TypeError);
    };
    let value = integer(ctx, *value)?;
    let count = integer(ctx, *count)?;
    let result = if count >= 0 {
        value
            .checked_shl(u32::try_from(count).map_err(|_| ObjectError::Layout)?)
            .ok_or(ObjectError::Layout)?
    } else {
        value
            .checked_shr(u32::try_from(count.unsigned_abs()).map_err(|_| ObjectError::Layout)?)
            .ok_or(ObjectError::Layout)?
    };
    integer_word(ctx, runtime, result)
}

/// Encode a byte specification as `(position << 32) | size`.
pub fn byte(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [size, position] = args else {
        return Err(ObjectError::TypeError);
    };
    let size = u32::try_from(size.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let position = u32::try_from(position.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    Ok(Word::from_bits(
        (u64::from(position) << 32) | u64::from(size),
    ))
}

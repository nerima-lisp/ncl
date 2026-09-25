//! Integer bit operations and byte manipulation.
//!
//! The callbacks in this module deliberately contain no symbol registration.  The
//! numbers registrar can install them alongside the other numeric callbacks.

use ncl_object::{
    Bignum, ObjectError, ObjectRef, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign,
    classify_object, make_bignum_from_i128,
};

use ncl_object::{BuiltinArgs, MultipleValues};

fn integer(ctx: &ThreadContext, value: Word) -> Result<i128, ObjectError> {
    if let Some(value) = value.as_fixnum() {
        return Ok(i128::from(value));
    }
    let ObjectRef::Bignum(word) = classify_object(ctx, value) else {
        return Err(ObjectError::TypeError);
    };
    let object = Bignum::from(word);
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

fn byte_parts(spec: Word) -> Result<(u32, u32), ObjectError> {
    let bits = spec.bits();
    Ok((
        u32::try_from(bits & u32::MAX as u64).map_err(|_| ObjectError::TypeError)?,
        u32::try_from(bits >> 32).map_err(|_| ObjectError::TypeError)?,
    ))
}

pub fn byte_size(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(Word::fixnum(i64::from(byte_parts(*spec)?.0)))
}

pub fn byte_position(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(Word::fixnum(i64::from(byte_parts(*spec)?.1)))
}

fn field(spec: Word, value: i128) -> Result<i128, ObjectError> {
    let (size, position) = byte_parts(spec)?;
    if size >= 127 || position >= 127 {
        return Err(ObjectError::Layout);
    }
    Ok((value >> position) & ((1_i128 << size) - 1))
}

pub fn ldb(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, field(*spec, integer(ctx, *value)?)?)
}

pub fn ldb_test(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(if field(*spec, integer(ctx, *value)?)? != 0 {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub fn mask_field(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    integer_word(
        ctx,
        runtime,
        field(*spec, -1)? << position & integer(ctx, *value)?,
    )
}

pub fn dpb(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [new_value, spec, old_value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    let old = integer(ctx, *old_value)?;
    let mask = field(*spec, -1)? << position;
    integer_word(
        ctx,
        runtime,
        (old & !mask) | ((integer(ctx, *new_value)? << position) & mask),
    )
}

pub fn deposit_field(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [new_value, spec, old_value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    let mask = field(*spec, -1)? << position;
    integer_word(
        ctx,
        runtime,
        (integer(ctx, *old_value)? & !mask) | (integer(ctx, *new_value)? & mask),
    )
}

pub const BOOLE_CLR: i64 = 0;
pub const BOOLE_1: i64 = 10;
pub const BOOLE_2: i64 = 12;
pub const BOOLE_C1: i64 = 5;
pub const BOOLE_C2: i64 = 3;
pub const BOOLE_AND: i64 = 8;
pub const BOOLE_IOR: i64 = 14;
pub const BOOLE_XOR: i64 = 6;
pub const BOOLE_EQV: i64 = 9;
pub const BOOLE_NAND: i64 = 7;
pub const BOOLE_NOR: i64 = 1;
pub const BOOLE_ANDC1: i64 = 4;
pub const BOOLE_ANDC2: i64 = 2;
pub const BOOLE_ORC1: i64 = 13;
pub const BOOLE_ORC2: i64 = 11;
pub const BOOLE_SET: i64 = 15;

pub fn boole(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [opcode, a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    let opcode = integer(ctx, *opcode)?;
    let a = integer(ctx, *a)?;
    let b = integer(ctx, *b)?;
    let result = match opcode {
        0 => 0,
        1 => !(a | b),
        2 => !a & !b,
        3 => !a,
        4 => a & !b,
        5 => !b,
        6 => a ^ b,
        7 => !(a & b),
        8 => a & b,
        9 => !(a ^ b),
        10 => a,
        11 => a | !b,
        12 => b,
        13 => !a | b,
        14 => a | b,
        15 => -1,
        _ => return Err(ObjectError::TypeError),
    };
    integer_word(ctx, runtime, result)
}

macro_rules! typed_dispatch {
    ($name:ident, $legacy:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $legacy(runtime, ctx, args.as_slice(), values)
        }
    };
}

typed_dispatch!(typed_logand, logand);
typed_dispatch!(typed_logior, logior);
typed_dispatch!(typed_logxor, logxor);
typed_dispatch!(typed_lognot, lognot);
typed_dispatch!(typed_logeqv, logeqv);
typed_dispatch!(typed_lognand, lognand);
typed_dispatch!(typed_lognor, lognor);
typed_dispatch!(typed_logandc1, logandc1);
typed_dispatch!(typed_logandc2, logandc2);
typed_dispatch!(typed_logorc1, logorc1);
typed_dispatch!(typed_logorc2, logorc2);
typed_dispatch!(typed_logtest, logtest);
typed_dispatch!(typed_logbitp, logbitp);
typed_dispatch!(typed_logcount, logcount);
typed_dispatch!(typed_integer_length, integer_length);
typed_dispatch!(typed_ash, ash);
typed_dispatch!(typed_byte, byte);
typed_dispatch!(typed_byte_size, byte_size);
typed_dispatch!(typed_byte_position, byte_position);
typed_dispatch!(typed_ldb, ldb);
typed_dispatch!(typed_dpb, dpb);
typed_dispatch!(typed_ldb_test, ldb_test);
typed_dispatch!(typed_mask_field, mask_field);
typed_dispatch!(typed_deposit_field, deposit_field);
typed_dispatch!(typed_boole, boole);

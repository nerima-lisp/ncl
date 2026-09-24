#![allow(clippy::useless_conversion)]

use crate::object_access::{fix, get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, number_offset, widetag, with_roots};
use ncl_sys::Word;

crate::word_newtype!(Bignum);
crate::word_newtype!(Ratio);
crate::word_newtype!(DoubleFloat);
crate::word_newtype!(Complex);

/// Allocate a bignum from a signed 128-bit integer.
///
/// # Errors
/// Returns an error when the encoded size cannot be represented.
pub fn make_bignum_from_i128(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: i128,
) -> Result<Bignum, ObjectError> {
    let sign = i64::from(value.is_negative());
    let mut rest = value.unsigned_abs();
    let mut limbs = Vec::new();
    while rest != 0 {
        limbs.push(u32::try_from(rest & u128::from(u32::MAX)).map_err(|_| ObjectError::Layout)?);
        rest >>= 32;
    }
    let object = allocate(
        ctx,
        runtime,
        widetag::BIGNUM,
        number_offset::LIMBS + limbs.len().div_ceil(2),
    )?;
    put(
        ctx,
        object,
        number_offset::SIGN,
        Word::from_bits(sign.cast_unsigned()),
    )?;
    put(ctx, object, number_offset::LIMB_COUNT, fix(limbs.len())?)?;
    for (index, pair) in limbs.chunks(2).enumerate() {
        put(
            ctx,
            object,
            number_offset::LIMBS + index,
            Word::from_bits(
                u64::from(pair[0]) | (pair.get(1).map_or(0, |limb| u64::from(*limb)) << 32),
            ),
        )?;
    }
    Ok(object.into())
}
/// Read bignum limbs.
///
/// # Errors
/// Returns an error when the object has invalid limb metadata.
pub fn bignum_limbs(ctx: &ThreadContext, object: Bignum) -> Result<Vec<u32>, ObjectError> {
    let count = usize::try_from(
        get(
            ctx,
            object.into(),
            widetag::BIGNUM,
            number_offset::LIMB_COUNT,
        )?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)?;
    (0..count)
        .map(|i| {
            let bits = get(
                ctx,
                object.into(),
                widetag::BIGNUM,
                number_offset::LIMBS + i / 2,
            )?
            .bits();
            Ok(if i % 2 == 0 {
                u32::try_from(bits & u64::from(u32::MAX)).map_err(|_| ObjectError::Layout)?
            } else {
                u32::try_from(bits >> 32).map_err(|_| ObjectError::Layout)?
            })
        })
        .collect()
}
/// Allocate a ratio.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_ratio(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    numerator: Word,
    denominator: Word,
) -> Result<Ratio, ObjectError> {
    with_roots(ctx, &[numerator, denominator], |ctx, values| {
        let object = allocate(ctx, runtime, widetag::RATIO, 2)?;
        put(ctx, object, number_offset::RATIO_NUMERATOR, *values[0])?;
        put(ctx, object, number_offset::RATIO_DENOMINATOR, *values[1])?;
        Ok(object.into())
    })
}
/// Allocate a binary64 object.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_double(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: f64,
) -> Result<DoubleFloat, ObjectError> {
    let object = allocate(ctx, runtime, widetag::DOUBLE_FLOAT, 1)?;
    put(
        ctx,
        object,
        number_offset::DOUBLE_BITS,
        Word::from_bits(value.to_bits()),
    )?;
    Ok(object.into())
}
/// Read a binary64 object.
///
/// # Errors
/// Returns an error when the object is not a double-float.
pub fn double_value(ctx: &ThreadContext, object: DoubleFloat) -> Result<f64, ObjectError> {
    Ok(f64::from_bits(
        get(
            ctx,
            object.into(),
            widetag::DOUBLE_FLOAT,
            number_offset::DOUBLE_BITS,
        )?
        .bits(),
    ))
}
/// Allocate a complex number.
///
/// # Errors
/// Returns an error when allocation fails.
pub fn make_complex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    real: Word,
    imag: Word,
) -> Result<Complex, ObjectError> {
    with_roots(ctx, &[real, imag], |ctx, values| {
        let object = allocate(ctx, runtime, widetag::COMPLEX, 2)?;
        put(ctx, object, number_offset::COMPLEX_REAL, *values[0])?;
        put(ctx, object, number_offset::COMPLEX_IMAG, *values[1])?;
        Ok(object.into())
    })
}
/// Read a ratio numerator.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn ratio_numerator(ctx: &ThreadContext, object: Ratio) -> Result<Word, ObjectError> {
    get(
        ctx,
        object.into(),
        widetag::RATIO,
        number_offset::RATIO_NUMERATOR,
    )
}
/// Read a ratio denominator.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn ratio_denominator(ctx: &ThreadContext, object: Ratio) -> Result<Word, ObjectError> {
    get(
        ctx,
        object.into(),
        widetag::RATIO,
        number_offset::RATIO_DENOMINATOR,
    )
}
/// Read a complex real component.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn complex_real(ctx: &ThreadContext, object: Complex) -> Result<Word, ObjectError> {
    get(
        ctx,
        object.into(),
        widetag::COMPLEX,
        number_offset::COMPLEX_REAL,
    )
}
/// Read a complex imaginary component.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn complex_imag(ctx: &ThreadContext, object: Complex) -> Result<Word, ObjectError> {
    get(
        ctx,
        object.into(),
        widetag::COMPLEX,
        number_offset::COMPLEX_IMAG,
    )
}
/// Read a bignum sign.
///
/// # Errors
/// Returns an error when the object is invalid.
pub fn bignum_sign(ctx: &ThreadContext, object: Bignum) -> Result<bool, ObjectError> {
    Ok(get(ctx, object.into(), widetag::BIGNUM, number_offset::SIGN)?.bits() != 0)
}

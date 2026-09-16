use super::HashTest;
use crate::{ObjectError, ThreadContext, string_length, string_ref, widetag};
use ncl_sys::Word;

pub(super) fn equal(
    ctx: &ThreadContext,
    test: HashTest,
    left: Word,
    right: Word,
    depth: usize,
) -> Result<bool, ObjectError> {
    if depth > 64 {
        return Ok(false);
    }
    match test {
        HashTest::Eq => Ok(left == right),
        HashTest::Eql => Ok(left == right || numeric_equal(ctx, left, right)?),
        HashTest::Equal | HashTest::Equalp => {
            let fold = test == HashTest::Equalp;
            if strings_equal(ctx, left, right, fold)? {
                return Ok(true);
            }
            if left.is_cons() || right.is_cons() {
                return Ok(left.is_cons()
                    && right.is_cons()
                    && equal(
                        ctx,
                        test,
                        cons_part(ctx, left, 0)?,
                        cons_part(ctx, right, 0)?,
                        depth + 1,
                    )?
                    && equal(
                        ctx,
                        test,
                        cons_part(ctx, left, 1)?,
                        cons_part(ctx, right, 1)?,
                        depth + 1,
                    )?);
            }
            Ok(left == right || numeric_equal(ctx, left, right)?)
        }
    }
}

fn strings_equal(
    ctx: &ThreadContext,
    left: Word,
    right: Word,
    fold: bool,
) -> Result<bool, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, left) != Some(widetag::STRING)
        || ncl_sys::object_widetag(&ctx.thread, right) != Some(widetag::STRING)
    {
        return Ok(false);
    }
    let length = string_length(ctx, left)?;
    if length != string_length(ctx, right)? {
        return Ok(false);
    }
    for index in 0..length {
        let left_char = string_ref(ctx, left, index)?;
        let right_char = string_ref(ctx, right, index)?;
        if (if fold {
            left_char.to_ascii_uppercase()
        } else {
            left_char
        }) != (if fold {
            right_char.to_ascii_uppercase()
        } else {
            right_char
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn numeric_equal(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    let tag = ncl_sys::object_widetag(&ctx.thread, left);
    if tag != ncl_sys::object_widetag(&ctx.thread, right) {
        return Ok(false);
    }
    Ok(match tag {
        Some(widetag::BIGNUM) => {
            crate::bignum_sign(ctx, left.into())? == crate::bignum_sign(ctx, right.into())?
                && crate::bignum_limbs(ctx, left.into())? == crate::bignum_limbs(ctx, right.into())?
        }
        Some(widetag::DOUBLE_FLOAT) => {
            crate::double_value(ctx, left.into())?.to_bits()
                == crate::double_value(ctx, right.into())?.to_bits()
        }
        Some(widetag::RATIO) => {
            numeric_equal(
                ctx,
                crate::ratio_numerator(ctx, left.into())?,
                crate::ratio_numerator(ctx, right.into())?,
            )? && numeric_equal(
                ctx,
                crate::ratio_denominator(ctx, left.into())?,
                crate::ratio_denominator(ctx, right.into())?,
            )?
        }
        Some(widetag::COMPLEX) => {
            numeric_equal(
                ctx,
                crate::complex_real(ctx, left.into())?,
                crate::complex_real(ctx, right.into())?,
            )? && numeric_equal(
                ctx,
                crate::complex_imag(ctx, left.into())?,
                crate::complex_imag(ctx, right.into())?,
            )?
        }
        _ => false,
    })
}

pub(super) fn hash_key(
    ctx: &ThreadContext,
    test: HashTest,
    word: Word,
) -> Result<u64, ObjectError> {
    match test {
        HashTest::Eq => Ok(crate::hash_table::sxhash(word)),
        HashTest::Eql => {
            Ok(numeric_hash(ctx, word)?.unwrap_or_else(|| crate::hash_table::sxhash(word)))
        }
        HashTest::Equal => content_hash(ctx, word, false, 0),
        HashTest::Equalp => content_hash(ctx, word, true, 0),
    }
}

fn content_hash(
    ctx: &ThreadContext,
    word: Word,
    fold: bool,
    depth: usize,
) -> Result<u64, ObjectError> {
    if depth > 64 {
        return Ok(0);
    }
    if ncl_sys::object_widetag(&ctx.thread, word) == Some(widetag::STRING) {
        let mut hash = 0xcbf2_9ce4_8422_2325;
        for index in 0..string_length(ctx, word)? {
            let character = string_ref(ctx, word, index)?;
            let character = if fold {
                character.to_ascii_uppercase()
            } else {
                character
            };
            hash = (hash ^ u64::from(character as u32)).wrapping_mul(0x0100_0000_01b3);
        }
        return Ok(hash);
    }
    if word.is_cons() {
        return Ok(
            content_hash(ctx, cons_part(ctx, word, 0)?, fold, depth + 1)?.rotate_left(7)
                ^ content_hash(ctx, cons_part(ctx, word, 1)?, fold, depth + 1)?,
        );
    }
    Ok(numeric_hash(ctx, word)?.unwrap_or_else(|| crate::hash_table::sxhash(word)))
}

fn cons_part(ctx: &ThreadContext, word: Word, slot: usize) -> Result<Word, ObjectError> {
    ncl_sys::read_cons_word(&ctx.thread, word, slot).ok_or(ObjectError::Storage(
        ncl_sys::StorageCondition::ThreadNotRegistered,
    ))
}

fn numeric_hash(ctx: &ThreadContext, word: Word) -> Result<Option<u64>, ObjectError> {
    Ok(match ncl_sys::object_widetag(&ctx.thread, word) {
        Some(widetag::BIGNUM) => Some(crate::bignum_limbs(ctx, word.into())?.into_iter().fold(
            u64::from(crate::bignum_sign(ctx, word.into())?),
            |hash, limb| hash.rotate_left(5) ^ u64::from(limb),
        )),
        Some(widetag::DOUBLE_FLOAT) => Some(crate::double_value(ctx, word.into())?.to_bits()),
        Some(widetag::RATIO) => Some(
            content_hash(ctx, crate::ratio_numerator(ctx, word.into())?, false, 0)?
                ^ content_hash(ctx, crate::ratio_denominator(ctx, word.into())?, false, 0)?
                    .rotate_left(11),
        ),
        Some(widetag::COMPLEX) => Some(
            content_hash(ctx, crate::complex_real(ctx, word.into())?, false, 0)?
                ^ content_hash(ctx, crate::complex_imag(ctx, word.into())?, false, 0)?
                    .rotate_left(11),
        ),
        _ => None,
    })
}

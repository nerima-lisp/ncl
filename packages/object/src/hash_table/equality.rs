use super::HashTest;
use crate::{string_length, string_ref, widetag, ObjectError, ThreadContext};
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
            crate::bignum_sign(ctx, crate::Bignum::from_word(left))? == crate::bignum_sign(ctx, crate::Bignum::from_word(right))?
                && crate::bignum_limbs(ctx, crate::Bignum::from_word(left))? == crate::bignum_limbs(ctx, crate::Bignum::from_word(right))?
        }
        Some(widetag::DOUBLE_FLOAT) => {
            crate::double_value(ctx, crate::DoubleFloat::from_word(left))?.to_bits()
                == crate::double_value(ctx, crate::DoubleFloat::from_word(right))?.to_bits()
        }
        Some(widetag::RATIO) => {
            numeric_equal(
                ctx,
                crate::ratio_numerator(ctx, crate::Ratio::from_word(left))?,
                crate::ratio_numerator(ctx, crate::Ratio::from_word(right))?,
            )? && numeric_equal(
                ctx,
                crate::ratio_denominator(ctx, crate::Ratio::from_word(left))?,
                crate::ratio_denominator(ctx, crate::Ratio::from_word(right))?,
            )?
        }
        Some(widetag::COMPLEX) => {
            numeric_equal(
                ctx,
                crate::complex_real(ctx, crate::Complex::from_word(left))?,
                crate::complex_real(ctx, crate::Complex::from_word(right))?,
            )? && numeric_equal(
                ctx,
                crate::complex_imag(ctx, crate::Complex::from_word(left))?,
                crate::complex_imag(ctx, crate::Complex::from_word(right))?,
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
        Some(widetag::BIGNUM) => Some(crate::bignum_limbs(ctx, crate::Bignum::from_word(word))?.into_iter().fold(
            u64::from(crate::bignum_sign(ctx, crate::Bignum::from_word(word))?),
            |hash, limb| hash.rotate_left(5) ^ u64::from(limb),
        )),
        Some(widetag::DOUBLE_FLOAT) => Some(crate::double_value(ctx, crate::DoubleFloat::from_word(word))?.to_bits()),
        Some(widetag::RATIO) => Some(
            content_hash(ctx, crate::ratio_numerator(ctx, crate::Ratio::from_word(word))?, false, 0)?
                ^ content_hash(ctx, crate::ratio_denominator(ctx, crate::Ratio::from_word(word))?, false, 0)?
                    .rotate_left(11),
        ),
        Some(widetag::COMPLEX) => Some(
            content_hash(ctx, crate::complex_real(ctx, crate::Complex::from_word(word))?, false, 0)?
                ^ content_hash(ctx, crate::complex_imag(ctx, crate::Complex::from_word(word))?, false, 0)?
                    .rotate_left(11),
        ),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{equal, hash_key};
    use crate::hash_table::HashTest;
    use crate::{
        make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio, make_string,
        Runtime, ThreadContext,
    };
    use ncl_sys::Word;

    #[test]
    fn equality_and_hashing_cover_strings_numbers_and_depth_limits() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut context = ThreadContext::new();
        context
            .register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        let left_string = make_string(&mut context, &runtime, &['a', 'b']).unwrap_or(Word::NIL);
        let right_string = make_string(&mut context, &runtime, &['A', 'B']).unwrap_or(Word::NIL);
        assert!(!equal(&context, HashTest::Equal, left_string, right_string, 0).unwrap());
        assert!(equal(&context, HashTest::Equalp, left_string, right_string, 0).unwrap());
        assert!(!equal(&context, HashTest::Equalp, left_string, Word::NIL, 0).unwrap());
        assert_eq!(
            equal(&context, HashTest::Equal, Word::NIL, Word::NIL, 65),
            Ok(false)
        );

        let bignum =
            make_bignum_from_i128(&mut context, &runtime, 1_234_567_890_123).unwrap_or(Word::NIL);
        let same_bignum =
            make_bignum_from_i128(&mut context, &runtime, 1_234_567_890_123).unwrap_or(Word::NIL);
        let double = make_double(&mut context, &runtime, 2.5).unwrap_or(Word::NIL);
        let same_double = make_double(&mut context, &runtime, 2.5).unwrap_or(Word::NIL);
        let ratio = make_ratio(&mut context, &runtime, bignum, double).unwrap_or(Word::NIL);
        let same_ratio =
            make_ratio(&mut context, &runtime, same_bignum, same_double).unwrap_or(Word::NIL);
        let complex = make_complex(&mut context, &runtime, ratio, bignum).unwrap_or(Word::NIL);
        let same_complex =
            make_complex(&mut context, &runtime, same_ratio, same_bignum).unwrap_or(Word::NIL);
        for (left, right) in [
            (bignum, same_bignum),
            (double, same_double),
            (ratio, same_ratio),
            (complex, same_complex),
        ] {
            assert!(equal(&context, HashTest::Eql, left, right, 0).unwrap());
            assert_eq!(
                hash_key(&context, HashTest::Eql, left),
                hash_key(&context, HashTest::Eql, right)
            );
        }
        assert!(equal(&context, HashTest::Equal, ratio, same_ratio, 0).unwrap());
        assert!(equal(&context, HashTest::Equalp, complex, same_complex, 0).unwrap());
        assert_eq!(
            hash_key(&context, HashTest::Equal, complex),
            hash_key(&context, HashTest::Equal, same_complex)
        );

        let left_cons = make_cons(&mut context, &runtime, left_string, ratio).unwrap_or(Word::NIL);
        let right_cons =
            make_cons(&mut context, &runtime, right_string, same_ratio).unwrap_or(Word::NIL);
        assert!(equal(&context, HashTest::Equalp, left_cons, right_cons, 0).unwrap());
        assert_eq!(
            hash_key(&context, HashTest::Equalp, left_cons),
            hash_key(&context, HashTest::Equalp, right_cons)
        );
        assert_eq!(
            hash_key(&context, HashTest::Eq, Word::fixnum(4)),
            Ok(crate::hash_table::sxhash(Word::fixnum(4)))
        );
    }
}

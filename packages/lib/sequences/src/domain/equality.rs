//! `EQUAL` and `EQUALP`.
//!
//! Both predicates are pure and read-only: they never allocate, so no
//! GC rooting is required around the comparisons themselves.

use ncl_object::hash_table::HashTable;
use ncl_object::{
    ArrayElementType, Bignum, Complex, DoubleFloat, LispError, ObjectError, ObjectRef, Ratio,
    Runtime, ThreadContext, Word, array_dimensions, array_row_major_ref, bignum_limbs,
    bignum_sign, car, cdr, classify_object, complex_imag, complex_real, double_value,
    ratio_denominator, ratio_numerator, simple_vector_length, simple_vector_ref,
    specialized_array_element_type, specialized_array_ref, string_length, string_ref,
};

/// Recursion depth limit shared with the hash-table content-equality helper,
/// guarding against circular structures.
const MAX_DEPTH: usize = 64;

/// `(equal x y)`.
///
/// # Errors
/// Propagates object-layer errors from reading either operand.
pub fn equal(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    equal_at(ctx, left, right, 0)
}

/// `(equalp x y)`.
///
/// # Errors
/// Propagates object-layer errors from reading either operand.
pub fn equalp(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    equalp_at(ctx, left, right, 0)
}

/// Builtin entry point for `(equal x y)`.
///
/// # Errors
/// Propagates object-layer errors from reading either operand.
pub fn equal_predicate(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    left: Word,
    right: Word,
) -> Result<Word, LispError> {
    Ok(if equal(ctx, left, right)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

/// Builtin entry point for `(equalp x y)`.
///
/// # Errors
/// Propagates object-layer errors from reading either operand.
pub fn equalp_predicate(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    left: Word,
    right: Word,
) -> Result<Word, LispError> {
    Ok(if equalp(ctx, left, right)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn equal_at(ctx: &ThreadContext, left: Word, right: Word, depth: usize) -> Result<bool, ObjectError> {
    if depth > MAX_DEPTH {
        return Ok(false);
    }
    if left == right {
        return Ok(true);
    }
    if left.is_cons() || right.is_cons() {
        return Ok(left.is_cons()
            && right.is_cons()
            && equal_at(ctx, car(ctx, left)?, car(ctx, right)?, depth + 1)?
            && equal_at(ctx, cdr(ctx, left)?, cdr(ctx, right)?, depth + 1)?);
    }
    match (classify_object(ctx, left), classify_object(ctx, right)) {
        (ObjectRef::String(_), ObjectRef::String(_)) => strings_equal(ctx, left, right, false),
        (
            ObjectRef::SpecializedArray(_) | ObjectRef::SimpleVector(_) | ObjectRef::Array(_),
            ObjectRef::SpecializedArray(_) | ObjectRef::SimpleVector(_) | ObjectRef::Array(_),
        ) => {
            // CLHS restricts EQUAL's elementwise array handling to bit
            // vectors; every other array kind falls back to identity.
            if specialized_bit_vector(ctx, left)? && specialized_bit_vector(ctx, right)? {
                arrays_equal(ctx, left, right, false, depth)
            } else {
                Ok(false)
            }
        }
        (
            ObjectRef::Bignum(_) | ObjectRef::Ratio(_) | ObjectRef::DoubleFloat(_)
            | ObjectRef::Complex(_) | ObjectRef::Fixnum(_),
            ObjectRef::Bignum(_) | ObjectRef::Ratio(_) | ObjectRef::DoubleFloat(_)
            | ObjectRef::Complex(_) | ObjectRef::Fixnum(_),
        ) => numeric_equal_exact(ctx, left, right),
        _ => Ok(false), // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive; remaining kinds compare by eq only.
    }
}

fn equalp_at(ctx: &ThreadContext, left: Word, right: Word, depth: usize) -> Result<bool, ObjectError> {
    if depth > MAX_DEPTH {
        return Ok(false);
    }
    if left == right {
        return Ok(true);
    }
    if is_number(ctx, left) && is_number(ctx, right) {
        return numeric_equal_loose(ctx, left, right);
    }
    if left.is_cons() || right.is_cons() {
        return Ok(left.is_cons()
            && right.is_cons()
            && equalp_at(ctx, car(ctx, left)?, car(ctx, right)?, depth + 1)?
            && equalp_at(ctx, cdr(ctx, left)?, cdr(ctx, right)?, depth + 1)?);
    }
    if let (ObjectRef::Character(a), ObjectRef::Character(b)) =
        (classify_object(ctx, left), classify_object(ctx, right))
    {
        return Ok(char_fold(a) == char_fold(b));
    }
    match (classify_object(ctx, left), classify_object(ctx, right)) {
        (ObjectRef::String(_), ObjectRef::String(_)) => strings_equal(ctx, left, right, true),
        (
            ObjectRef::SpecializedArray(_) | ObjectRef::SimpleVector(_) | ObjectRef::Array(_)
            | ObjectRef::String(_),
            ObjectRef::SpecializedArray(_) | ObjectRef::SimpleVector(_) | ObjectRef::Array(_)
            | ObjectRef::String(_),
        ) => arrays_equal(ctx, left, right, true, depth),
        (ObjectRef::HashTable(_), ObjectRef::HashTable(_)) => {
            hash_tables_equal(ctx, left, right, depth)
        }
        _ => Ok(false), // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive; remaining kinds compare by eq only.
    }
}

fn char_fold(code: u32) -> u32 {
    char::from_u32(code).map_or(code, |c| u32::from(c.to_ascii_uppercase()))
}

fn is_number(ctx: &ThreadContext, word: Word) -> bool {
    matches!(
        classify_object(ctx, word),
        ObjectRef::Fixnum(_)
            | ObjectRef::Bignum(_)
            | ObjectRef::Ratio(_)
            | ObjectRef::DoubleFloat(_)
            | ObjectRef::Complex(_)
    )
}

fn strings_equal(ctx: &ThreadContext, left: Word, right: Word, fold: bool) -> Result<bool, ObjectError> {
    let length = string_length(ctx, left)?;
    if length != string_length(ctx, right)? {
        return Ok(false);
    }
    for index in 0..length {
        let a = string_ref(ctx, left, index)?;
        let b = string_ref(ctx, right, index)?;
        let matches = if fold {
            a.to_ascii_uppercase() == b.to_ascii_uppercase()
        } else {
            a == b
        };
        if !matches {
            return Ok(false);
        }
    }
    Ok(true)
}

fn specialized_bit_vector(ctx: &ThreadContext, word: Word) -> Result<bool, ObjectError> {
    if let ObjectRef::SpecializedArray(_) = classify_object(ctx, word) {
        Ok(specialized_array_element_type(ctx, word)? == ArrayElementType::Bit)
    } else {
        Ok(false)
    }
}

/// Shape (dimensions) of any array-like object: string, simple vector,
/// specialized array, or general array.
fn shape_of(ctx: &ThreadContext, word: Word) -> Result<Vec<usize>, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::String(value) => Ok(vec![string_length(ctx, value)?]),
        ObjectRef::SimpleVector(value) => Ok(vec![simple_vector_length(ctx, value)?]),
        ObjectRef::SpecializedArray(_) | ObjectRef::Array(_) => array_dimensions(ctx, word),
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) non-array kinds are rejected.
    }
}

/// Row-major element access for any array-like object.
fn element_of(ctx: &ThreadContext, word: Word, index: usize) -> Result<Word, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::String(value) => {
            string_ref(ctx, value, index).map(|c| Word::character(u32::from(c)))
        }
        ObjectRef::SimpleVector(value) => simple_vector_ref(ctx, value, index),
        ObjectRef::SpecializedArray(_) => specialized_array_ref(ctx, word, index),
        ObjectRef::Array(_) => array_row_major_ref(ctx, word, index),
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) non-array kinds are rejected.
    }
}

fn arrays_equal(
    ctx: &ThreadContext,
    left: Word,
    right: Word,
    fold: bool,
    depth: usize,
) -> Result<bool, ObjectError> {
    let left_shape = shape_of(ctx, left)?;
    let right_shape = shape_of(ctx, right)?;
    if left_shape != right_shape {
        return Ok(false);
    }
    let total = left_shape
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    for index in 0..total {
        let a = element_of(ctx, left, index)?;
        let b = element_of(ctx, right, index)?;
        let matches = if fold {
            equalp_at(ctx, a, b, depth + 1)?
        } else {
            equal_at(ctx, a, b, depth + 1)?
        };
        if !matches {
            return Ok(false);
        }
    }
    Ok(true)
}

fn hash_tables_equal(
    ctx: &ThreadContext,
    left: Word,
    right: Word,
    depth: usize,
) -> Result<bool, ObjectError> {
    let left = HashTable::from_word(left);
    let right = HashTable::from_word(right);
    if left.test(ctx)? != right.test(ctx)? || left.count(ctx)? != right.count(ctx)? {
        return Ok(false);
    }
    let mut entries = Vec::with_capacity(left.count(ctx)?);
    left.for_each_entry(ctx, |key, value| entries.push((key, value)))?;
    for (key, value) in entries {
        // `HashTable::get` requires `&mut ThreadContext` in its general
        // signature; equality only reads, so a short-lived mutable borrow of
        // a throwaway clone would be unsound. Instead we linearly scan
        // `right`'s entries, which keeps this to a read-only pass.
        let mut found = false;
        let mut matched = false;
        right.for_each_entry(ctx, |other_key, other_value| {
            if !found {
                // Best-effort equality: two keys are treated as the same
                // entry when they are `eq`, `eql`, or content-equal, which
                // covers every standard hash-table test.
                let same_key = other_key == key
                    || equal_at(ctx, other_key, key, depth + 1).unwrap_or(false);
                if same_key {
                    found = true;
                    matched = equalp_at(ctx, other_value, value, depth + 1).unwrap_or(false);
                }
            }
        })?;
        if !found || !matched {
            return Ok(false);
        }
    }
    Ok(true)
}

fn numeric_equal_exact(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    match (classify_object(ctx, left), classify_object(ctx, right)) {
        (ObjectRef::Fixnum(a), ObjectRef::Fixnum(b)) => Ok(a == b),
        (ObjectRef::Bignum(_), ObjectRef::Bignum(_)) => Ok(bignum_sign(ctx, Bignum::from_word(left))?
            == bignum_sign(ctx, Bignum::from_word(right))?
            && bignum_limbs(ctx, Bignum::from_word(left))? == bignum_limbs(ctx, Bignum::from_word(right))?),
        (ObjectRef::DoubleFloat(_), ObjectRef::DoubleFloat(_)) => {
            Ok(double_value(ctx, DoubleFloat::from_word(left))?.to_bits() == double_value(ctx, DoubleFloat::from_word(right))?.to_bits())
        }
        (ObjectRef::Ratio(_), ObjectRef::Ratio(_)) => Ok(numeric_equal_exact(
            ctx,
            ratio_numerator(ctx, Ratio::from_word(left))?,
            ratio_numerator(ctx, Ratio::from_word(right))?,
        )? && numeric_equal_exact(
            ctx,
            ratio_denominator(ctx, Ratio::from_word(left))?,
            ratio_denominator(ctx, Ratio::from_word(right))?,
        )?),
        (ObjectRef::Complex(_), ObjectRef::Complex(_)) => Ok(numeric_equal_exact(
            ctx,
            complex_real(ctx, Complex::from_word(left))?,
            complex_real(ctx, Complex::from_word(right))?,
        )? && numeric_equal_exact(
            ctx,
            complex_imag(ctx, Complex::from_word(left))?,
            complex_imag(ctx, Complex::from_word(right))?,
        )?),
        _ => Ok(false), // check-added-lines: allow(wildcard) mismatched or non-numeric kinds are not equal.
    }
}

/// Convert any real number to an `f64` for cross-type `EQUALP` comparison.
///
/// This is exact for fixnums, doubles, and bignums/ratios that fit in an
/// `f64` mantissa; extremely large bignums or ratios lose precision, which is
/// an accepted trade-off given the absence of an arbitrary-precision rational
/// comparator in this crate.
fn real_to_f64(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, word) {
        #[allow(clippy::cast_precision_loss)]
        ObjectRef::Fixnum(value) => Ok(value as f64), // check-added-lines: allow(as-cast) approximate cross-type EQUALP comparison.
        ObjectRef::DoubleFloat(_) => double_value(ctx, DoubleFloat::from_word(word)),
        ObjectRef::Bignum(_) => Ok(bignum_limbs(ctx, Bignum::from_word(word))?
            .iter()
            .rev()
            .fold(0.0_f64, |accumulator, limb| {
                accumulator.mul_add(f64::from(u32::MAX) + 1.0, f64::from(*limb))
            })
            * if bignum_sign(ctx, Bignum::from_word(word))? { -1.0 } else { 1.0 }),
        ObjectRef::Ratio(_) => {
            let numerator = real_to_f64(ctx, ratio_numerator(ctx, Ratio::from_word(word))?)?;
            let denominator = real_to_f64(ctx, ratio_denominator(ctx, Ratio::from_word(word))?)?;
            Ok(numerator / denominator)
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) only real-numeric kinds convert.
    }
}

fn numeric_equal_loose(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    if let (ObjectRef::Complex(_), ObjectRef::Complex(_)) =
        (classify_object(ctx, left), classify_object(ctx, right))
    {
        return Ok(numeric_equal_loose(
            ctx,
            complex_real(ctx, Complex::from_word(left))?,
            complex_real(ctx, Complex::from_word(right))?,
        )? && numeric_equal_loose(
            ctx,
            complex_imag(ctx, Complex::from_word(left))?,
            complex_imag(ctx, Complex::from_word(right))?,
        )?);
    }
    if matches!(classify_object(ctx, left), ObjectRef::Complex(_))
        || matches!(classify_object(ctx, right), ObjectRef::Complex(_))
    {
        // A complex is only `equalp` to a non-complex if its imaginary part
        // is (numerically) zero; approximate this with the same fallback.
        let (complex, other) = if matches!(classify_object(ctx, left), ObjectRef::Complex(_)) {
            (left, right)
        } else {
            (right, left)
        };
        return Ok(real_to_f64(ctx, complex_imag(ctx, Complex::from_word(complex))?)? == 0.0
            && numeric_equal_loose(ctx, complex_real(ctx, Complex::from_word(complex))?, other)?);
    }
    // Same exact representation compares exactly first, so huge bignums and
    // ratios of matching type never lose precision.
    if numeric_equal_exact(ctx, left, right)? {
        return Ok(true);
    }
    Ok((real_to_f64(ctx, left)? - real_to_f64(ctx, right)?).abs() == 0.0)
}

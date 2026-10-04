use super::{HashTable, HashTest, Weakness};
use crate::object_access::{get, put};
use crate::{
    Bignum, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, bignum_limbs, bignum_sign,
    classify_object, double_value, finish_root, make_double, ratio_denominator, ratio_numerator,
    widetag,
};
use ncl_sys::Word;

const DEFAULT_CAPACITY: usize = 8;
const DEFAULT_REHASH_SIZE: f64 = 1.5;
const DEFAULT_REHASH_THRESHOLD: f64 = 0.75;
pub(super) fn refresh_forwarded_field(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: HashTable,
    slot: usize,
) -> Result<Word, ObjectError> {
    let value = get(ctx, table.as_word(), widetag::HASH_TABLE, slot)?;
    if matches!(
        crate::classify(value),
        ObjectRef::Fixnum(_) | ObjectRef::Character(_) | ObjectRef::Immediate(_)
    ) {
        return Ok(value);
    }
    let forwarded = runtime
        .heap
        .forwarded_word(value)
        .ok_or(ObjectError::Layout)?;
    if forwarded != value {
        put(ctx, table.as_word(), slot, forwarded)?;
    }
    Ok(forwarded)
}
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
            let value = double_value(ctx, crate::DoubleFloat::from_word(value))?;
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
        // check-added-lines: allow(wildcard) ObjectRef is non_exhaustive and unknown values are invalid.
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

pub(super) fn next_capacity_from_value(
    ctx: &ThreadContext,
    table: HashTable,
    rehash_size: Word,
) -> Result<usize, ObjectError> {
    let capacity = table.capacity(ctx)?;
    let requested = match rehash_size_value(ctx, rehash_size)? {
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

#[cfg(test)]
mod tests {
    use super::{
        float_ratio, next_capacity_from_value, normalize_capacity, rehash_size_value,
        rehash_threshold_value, validate_rehash_size, validate_rehash_threshold,
    };
    use crate::hash_table::{HashTable, HashTest, Weakness};
    use crate::{
        ObjectError, Runtime, ThreadContext, make_bignum_from_i128, make_double, make_ratio,
    };
    use ncl_sys::Word;

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        (runtime, ctx)
    }
    #[test]
    fn option_values_validate_numeric_boundaries() {
        assert_eq!(normalize_capacity(1), Ok(8));
        assert_eq!(normalize_capacity(8), Ok(8));
        assert_eq!(normalize_capacity(9), Ok(16));
        assert_eq!(
            normalize_capacity((usize::MAX / 2 + 1) as u128),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            normalize_capacity(usize::MAX as u128),
            Err(ObjectError::TypeError)
        );
        assert_eq!(normalize_capacity(u128::MAX), Err(ObjectError::TypeError));
        let (runtime, mut ctx) = setup();
        let positive_bignum = make_bignum_from_i128(&mut ctx, &runtime, 17)
            .unwrap_or_else(|error| panic!("bignum: {error:?}"));
        assert_eq!(validate_rehash_size(&ctx, positive_bignum.into()), Ok(()));
        let factor = make_double(&mut ctx, &runtime, 2.0)
            .unwrap_or_else(|error| panic!("factor: {error:?}"));
        assert_eq!(validate_rehash_size(&ctx, factor.into()), Ok(()));
        assert_eq!(validate_rehash_size(&ctx, Word::fixnum(1)), Ok(()));
        assert_eq!(
            validate_rehash_size(&ctx, Word::fixnum(0)),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            validate_rehash_size(&ctx, Word::fixnum(-1)),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            validate_rehash_size(&ctx, Word::TRUE),
            Err(ObjectError::TypeError)
        );
        let half = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
            .unwrap_or_else(|error| panic!("ratio: {error:?}"));
        assert_eq!(validate_rehash_threshold(&ctx, half.into()), Ok(()));
        assert_eq!(
            validate_rehash_threshold(&ctx, Word::fixnum(0)),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            validate_rehash_threshold(&ctx, Word::fixnum(2)),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            validate_rehash_threshold(&ctx, Word::TRUE),
            Err(ObjectError::TypeError)
        );
    }
    #[test]
    fn option_conversions_cover_numeric_forms_and_layout_errors() {
        let (runtime, mut ctx) = setup();
        let negative = make_bignum_from_i128(&mut ctx, &runtime, -1)
            .unwrap_or_else(|error| panic!("negative bignum: {error:?}"));
        let zero = make_bignum_from_i128(&mut ctx, &runtime, 0)
            .unwrap_or_else(|error| panic!("zero bignum: {error:?}"));
        assert_eq!(
            validate_rehash_size(&ctx, negative.into()),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            validate_rehash_size(&ctx, zero.into()),
            Err(ObjectError::TypeError)
        );
        let factor = make_double(&mut ctx, &runtime, 1.5)
            .unwrap_or_else(|error| panic!("factor: {error:?}"));
        assert!(
            matches!(rehash_size_value(&ctx, factor.into(),), Ok(super::RehashSize::Multiply(value)) if (value - 1.5).abs() < f64::EPSILON)
        );
        assert!(matches!(
            rehash_size_value(&ctx, Word::fixnum(3)),
            Ok(super::RehashSize::Add(3))
        ));
        assert!(matches!(
            rehash_size_value(&ctx, Word::NIL),
            Err(ObjectError::Layout)
        ));
        assert!(matches!(
            rehash_size_value(&ctx, Word::fixnum(0)),
            Err(ObjectError::TypeError)
        ));
        let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-1), Word::fixnum(2))
            .unwrap_or_else(|error| panic!("ratio: {error:?}"));
        assert_eq!(rehash_threshold_value(&ctx, ratio.into()), Ok(-0.5));
        let bignum = make_bignum_from_i128(&mut ctx, &runtime, -17)
            .unwrap_or_else(|error| panic!("threshold bignum: {error:?}"));
        assert_eq!(rehash_threshold_value(&ctx, bignum.into()), Ok(-17.0));
        assert_eq!(
            rehash_threshold_value(&ctx, Word::NIL),
            Err(ObjectError::TypeError)
        );
        assert_eq!(float_ratio(4.0), Ok((1_u128 << 52, 50)));
        assert_eq!(float_ratio(1.5), Ok((3_u128 << 51, 52)));
        assert_eq!(float_ratio(f64::from_bits(1)), Ok((1, 1074)));
        let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        assert_eq!(
            next_capacity_from_value(&ctx, table, Word::fixnum(3)),
            Ok(16)
        );
        assert_eq!(next_capacity_from_value(&ctx, table, factor.into()), Ok(16));
        assert_eq!(
            next_capacity_from_value(&ctx, table, Word::TRUE),
            Err(ObjectError::Layout)
        );
        assert_eq!(rehash_threshold_value(&ctx, Word::fixnum(3)), Ok(3.0));
        let positive_threshold = make_double(&mut ctx, &runtime, 0.75)
            .unwrap_or_else(|error| panic!("threshold: {error:?}"));
        assert_eq!(
            validate_rehash_threshold(&ctx, positive_threshold.into()),
            Ok(())
        );
        let too_large_threshold = make_double(&mut ctx, &runtime, 1.5)
            .unwrap_or_else(|error| panic!("threshold: {error:?}"));
        assert_eq!(
            validate_rehash_threshold(&ctx, too_large_threshold.into()),
            Err(ObjectError::TypeError)
        );
        let invalid_factor = make_double(&mut ctx, &runtime, 1.0)
            .unwrap_or_else(|error| panic!("factor: {error:?}"));
        assert!(matches!(
            rehash_size_value(&ctx, invalid_factor.into()),
            Err(ObjectError::Layout)
        ));
        assert_eq!(float_ratio(2.0), Ok((1_u128 << 52, 51)));
    }
    #[test]
    fn table_options_are_stored_and_defaults_are_observable() {
        let (runtime, mut ctx) = setup();
        let default_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::Key)
            .unwrap_or_else(|error| panic!("default table: {error:?}"));
        assert_eq!(default_table.test(&ctx), Ok(HashTest::Equalp));
        assert_eq!(default_table.weakness(&ctx), Ok(Weakness::Key));
        assert_eq!(default_table.capacity(&ctx), Ok(8));
        assert_eq!(default_table.count(&ctx), Ok(0));
        let size = Word::fixnum(9);
        let rehash_size = Word::fixnum(3);
        let rehash_threshold = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
            .unwrap_or_else(|error| panic!("threshold: {error:?}"));
        let table = HashTable::new_with_options(
            &mut ctx,
            &runtime,
            HashTest::Eq,
            Weakness::Value,
            size,
            Some(rehash_size),
            Some(rehash_threshold.into()),
        )
        .unwrap_or_else(|error| panic!("configured table: {error:?}"));
        assert_eq!(table.capacity(&ctx), Ok(16));
        assert_eq!(table.rehash_size(&ctx), Ok(rehash_size));
        let threshold = table
            .rehash_threshold(&ctx)
            .unwrap_or_else(|error| panic!("threshold field: {error:?}"));
        assert_eq!(rehash_threshold_value(&ctx, threshold), Ok(0.5));
    }
}

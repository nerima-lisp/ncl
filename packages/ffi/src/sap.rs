//! System area pointers and the arithmetic over them.
//!
//! A SAP is represented in Lisp by a non-negative integer holding its machine
//! address, so `sap-int` and `int-sap` are exact inverses. The runtime does not
//! provide a distinct SAP object kind, so SAP values and integer addresses are
//! interchangeable.

use ncl_object::{
    Bignum, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign, make_bignum_from_i128,
    make_bignum_from_limbs,
};

use crate::FfiError;

/// A raw system area pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SystemAreaPointer(usize);

impl SystemAreaPointer {
    /// Wrap a machine address.
    #[must_use]
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    /// The null pointer.
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Machine address passed to foreign calls and used for SAP arithmetic.
    #[must_use]
    pub const fn address(self) -> usize {
        self.0
    }

    /// Check the C null-pointer convention, where address zero is null.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Add a byte offset, wrapping like `sap+`.
    #[must_use]
    pub const fn sap_plus(self, offset: isize) -> Self {
        Self(self.0.wrapping_add_signed(offset))
    }

    /// Subtract a byte offset, wrapping like `sap-`.
    #[must_use]
    pub const fn sap_minus(self, offset: isize) -> Self {
        Self(self.0.wrapping_add_signed(offset.wrapping_neg()))
    }

    /// The signed byte distance to `other`, like `sap-` on two pointers.
    #[must_use]
    pub const fn sap_difference(self, other: Self) -> isize {
        self.0.wrapping_sub(other.0).cast_signed()
    }

    /// Encode as the exact non-negative Lisp integer holding the address.
    ///
    /// # Errors
    /// Returns an error if creating the Lisp integer fails.
    pub fn to_word(self, ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, FfiError> {
        unsigned_integer_to_word(
            ctx,
            runtime,
            u128::try_from(self.0).map_err(|_| FfiError::ValueOutOfRange {
                type_name: "system-area-pointer",
            })?,
        )
    }

    /// Decode a Lisp integer address, like `int-sap`.
    ///
    /// # Errors
    /// Returns [`FfiError::TypeMismatch`] when `value` is not an integer and
    /// [`FfiError::ValueOutOfRange`] when it is negative or exceeds the machine
    /// address range.
    pub fn from_word(ctx: &ThreadContext, value: Word) -> Result<Self, FfiError> {
        let address = unsigned_integer_from_word(ctx, value, "system-area-pointer")?;
        let address = usize::try_from(address).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "system-area-pointer",
        })?;
        Ok(Self(address))
    }
}

const MOST_POSITIVE_FIXNUM: i128 = (1_i128 << 62) - 1;
const MOST_NEGATIVE_FIXNUM: i128 = -(1_i128 << 62);
const MOST_POSITIVE_UNSIGNED_FIXNUM: u128 = (1_u128 << 62) - 1;

pub(crate) fn signed_integer_to_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: i128,
) -> Result<Word, FfiError> {
    if (MOST_NEGATIVE_FIXNUM..=MOST_POSITIVE_FIXNUM).contains(&value) {
        return i64::try_from(value)
            .map(Word::fixnum)
            .map_err(|_| FfiError::ValueOutOfRange {
                type_name: "integer",
            });
    }
    Ok(make_bignum_from_i128(ctx, runtime, value)?.as_word())
}

fn unsigned_integer_to_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: u128,
) -> Result<Word, FfiError> {
    if value <= MOST_POSITIVE_UNSIGNED_FIXNUM {
        return i64::try_from(value)
            .map(Word::fixnum)
            .map_err(|_| FfiError::ValueOutOfRange {
                type_name: "system-area-pointer",
            });
    }
    let mut rest = value;
    let mut limbs = Vec::new();
    while rest != 0 {
        limbs.push(u32::try_from(rest & u128::from(u32::MAX)).map_err(|_| {
            FfiError::ValueOutOfRange {
                type_name: "system-area-pointer",
            }
        })?);
        rest >>= 32;
    }
    Ok(make_bignum_from_limbs(ctx, runtime, false, &limbs)?.as_word())
}

fn unsigned_integer_from_word(
    ctx: &ThreadContext,
    value: Word,
    type_name: &'static str,
) -> Result<u128, FfiError> {
    if let Some(fixnum) = value.as_fixnum() {
        return u128::try_from(fixnum).map_err(|_| FfiError::ValueOutOfRange { type_name });
    }
    let bignum = Bignum::from_word(value);
    let negative = bignum_sign(ctx, bignum).map_err(|_| FfiError::TypeMismatch { type_name })?;
    if negative {
        return Err(FfiError::ValueOutOfRange { type_name });
    }
    bignum_limbs(ctx, bignum)
        .map_err(|_| FfiError::TypeMismatch { type_name })?
        .into_iter()
        .rev()
        .try_fold(0_u128, |value, limb| {
            value
                .checked_mul(1_u128 << 32)
                .and_then(|value| value.checked_add(u128::from(limb)))
                .ok_or(FfiError::ValueOutOfRange { type_name })
        })
}

/// Whether two pointers are equal, like `sap=`.
#[must_use]
pub fn sap_eq(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left == right
}

/// Whether `left` addresses a lower byte than `right`, like `sap<`.
#[must_use]
pub const fn sap_lt(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() < right.address()
}

/// Whether `left` addresses a lower or equal byte, like `sap<=`.
#[must_use]
pub const fn sap_le(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() <= right.address()
}

/// Whether `left` addresses a higher byte than `right`, like `sap>`.
#[must_use]
pub const fn sap_gt(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() > right.address()
}

/// Whether `left` addresses a higher or equal byte, like `sap>=`.
#[must_use]
pub const fn sap_ge(left: SystemAreaPointer, right: SystemAreaPointer) -> bool {
    left.address() >= right.address()
}

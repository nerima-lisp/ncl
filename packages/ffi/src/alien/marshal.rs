//! Marshalling between Lisp values and the foreign byte representation.
//!
//! The byte order is little-endian, matching the supported targets (x86-64 and
//! `AArch64` on Linux and macOS).

use ncl_object::{
    Bignum, DoubleFloat, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign, double_value,
    make_bignum_from_i128, make_double,
};

use super::{AlienType, size_of};
use crate::FfiError;
use crate::sap::SystemAreaPointer;
use crate::sys_requirements::READ_SYSTEM_MEMORY;

/// Largest fixnum, matching the 63-bit tagged payload.
const MOST_POSITIVE_FIXNUM: i128 = (1_i128 << 62) - 1;

/// Most negative fixnum.
const MOST_NEGATIVE_FIXNUM: i128 = -(1_i128 << 62);

/// Largest fixnum as an unsigned magnitude.
const MOST_POSITIVE_FIXNUM_UNSIGNED: u128 = (1_u128 << 62) - 1;

/// Marshal `value` into the foreign byte representation of `ty`.
///
/// # Errors
/// Returns [`FfiError::TypeMismatch`] or [`FfiError::ValueOutOfRange`] when the
/// value does not fit the declared type, [`FfiError::UnsupportedType`] for a
/// type with no Phase-1 representation, and [`FfiError::MissingSysPrimitive`]
/// for a composite type whose memory layout cannot be read yet.
pub fn marshal_argument(
    ctx: &ThreadContext,
    ty: &AlienType,
    value: Word,
) -> Result<Vec<u8>, FfiError> {
    match ty {
        AlienType::Void => Err(FfiError::UnsupportedType("void")),
        AlienType::Boolean => Ok(vec![u8::from(boolean_value(value)?)]),
        AlienType::Char | AlienType::UnsignedChar => {
            let scalar = character_value(value)?;
            let byte = u8::try_from(scalar).map_err(|_| FfiError::ValueOutOfRange {
                type_name: ty.label(),
            })?;
            Ok(vec![byte])
        }
        AlienType::Short => signed_bytes(word_to_i128(ctx, value)?, 2, ty.label()),
        AlienType::UnsignedShort => unsigned_bytes(word_to_u128(ctx, value)?, 2, ty.label()),
        AlienType::Int | AlienType::Enumeration(_) => {
            signed_bytes(word_to_i128(ctx, value)?, 4, ty.label())
        }
        AlienType::UnsignedInt => unsigned_bytes(word_to_u128(ctx, value)?, 4, ty.label()),
        AlienType::Long | AlienType::LongLong | AlienType::SSizeT => {
            signed_bytes(word_to_i128(ctx, value)?, 8, ty.label())
        }
        AlienType::UnsignedLong | AlienType::UnsignedLongLong | AlienType::SizeT => {
            unsigned_bytes(word_to_u128(ctx, value)?, 8, ty.label())
        }
        AlienType::SingleFloat => Ok(single_value(ctx, value)?.to_le_bytes().to_vec()),
        AlienType::DoubleFloat => Ok(double_value_of(ctx, value)?.to_le_bytes().to_vec()),
        AlienType::LongFloat => Err(FfiError::UnsupportedType("long-float")),
        AlienType::Pointer(_)
        | AlienType::CString
        | AlienType::Utf8String
        | AlienType::SystemAreaPointer
        | AlienType::Function(_) => Ok(pointer_address(value)?.to_le_bytes().to_vec()),
        AlienType::Array(_, _) | AlienType::Structure(_) | AlienType::Union(_) => {
            Err(FfiError::MissingSysPrimitive(READ_SYSTEM_MEMORY))
        }
    }
}

/// Unmarshal `bytes`, the foreign result of `ty`, into a Lisp value.
///
/// # Errors
/// Returns [`FfiError::ValueOutOfRange`] when `bytes` does not have the width of
/// `ty`, [`FfiError::UnsupportedType`] for a type with no Phase-1
/// representation, and [`FfiError::MissingSysPrimitive`] for a composite result
/// whose memory layout cannot be read yet.
pub fn unmarshal_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    ty: &AlienType,
    bytes: &[u8],
) -> Result<Word, FfiError> {
    if matches!(ty, AlienType::Void) {
        return Ok(Word::NIL);
    }
    if bytes.len() != size_of(ty) {
        return Err(FfiError::ValueOutOfRange {
            type_name: ty.label(),
        });
    }
    match ty {
        AlienType::Void => Ok(Word::NIL),
        AlienType::Boolean => Ok(if read_byte(bytes)? == 0 {
            Word::NIL
        } else {
            Word::TRUE
        }),
        AlienType::Char | AlienType::UnsignedChar => {
            Ok(Word::character(u32::from(read_byte(bytes)?)))
        }
        AlienType::Short
        | AlienType::Int
        | AlienType::Long
        | AlienType::LongLong
        | AlienType::SSizeT
        | AlienType::Enumeration(_) => i128_to_word(ctx, runtime, read_signed(bytes)?),
        AlienType::UnsignedShort
        | AlienType::UnsignedInt
        | AlienType::UnsignedLong
        | AlienType::UnsignedLongLong
        | AlienType::SizeT => u128_to_word(ctx, runtime, read_unsigned(bytes)?),
        AlienType::SingleFloat => {
            let raw: [u8; 4] = bytes.try_into().map_err(|_| FfiError::ValueOutOfRange {
                type_name: "single-float",
            })?;
            let bits = u64::from(f32::from_le_bytes(raw).to_bits());
            Ok(Word::from_bits(
                (bits << 4) | u64::from(LOWTAG_SINGLE_FLOAT),
            ))
        }
        AlienType::DoubleFloat => {
            let raw: [u8; 8] = bytes.try_into().map_err(|_| FfiError::ValueOutOfRange {
                type_name: "double-float",
            })?;
            Ok(make_double(ctx, runtime, f64::from_le_bytes(raw))?.as_word())
        }
        AlienType::LongFloat => Err(FfiError::UnsupportedType("long-float")),
        AlienType::Pointer(_)
        | AlienType::CString
        | AlienType::Utf8String
        | AlienType::SystemAreaPointer
        | AlienType::Function(_) => {
            let address =
                usize::try_from(read_unsigned(bytes)?).map_err(|_| FfiError::ValueOutOfRange {
                    type_name: "pointer",
                })?;
            if address == 0 {
                Ok(Word::NIL)
            } else {
                Ok(SystemAreaPointer::new(address).as_word())
            }
        }
        AlienType::Array(_, _) | AlienType::Structure(_) | AlienType::Union(_) => {
            Err(FfiError::MissingSysPrimitive(READ_SYSTEM_MEMORY))
        }
    }
}

/// Encode a signed value in `width` little-endian bytes.
fn signed_bytes(value: i128, width: usize, type_name: &'static str) -> Result<Vec<u8>, FfiError> {
    let bits = u32::try_from(width * 8).map_err(|_| FfiError::UnsupportedType("integer"))?;
    let minimum = -(1_i128 << (bits - 1));
    let maximum = (1_i128 << (bits - 1)) - 1;
    if value < minimum || value > maximum {
        return Err(FfiError::ValueOutOfRange { type_name });
    }
    Ok(value
        .cast_unsigned()
        .to_le_bytes()
        .into_iter()
        .take(width)
        .collect())
}

/// Encode an unsigned value in `width` little-endian bytes.
fn unsigned_bytes(value: u128, width: usize, type_name: &'static str) -> Result<Vec<u8>, FfiError> {
    let bits = width * 8;
    if bits < 128 && value >= (1_u128 << bits) {
        return Err(FfiError::ValueOutOfRange { type_name });
    }
    Ok(value.to_le_bytes().into_iter().take(width).collect())
}

/// Decode a signed value from little-endian bytes of any width up to 16.
fn read_signed(bytes: &[u8]) -> Result<i128, FfiError> {
    let mut buffer = [0_u8; 16];
    if bytes.len() > buffer.len() {
        return Err(FfiError::UnsupportedType("integer"));
    }
    for (destination, source) in buffer.iter_mut().zip(bytes) {
        *destination = *source;
    }
    let raw = i128::from_le_bytes(buffer);
    let width = u32::try_from(bytes.len() * 8).map_err(|_| FfiError::UnsupportedType("integer"))?;
    let shift = 128_u32.saturating_sub(width);
    if shift == 128 {
        return Ok(0);
    }
    Ok((raw << shift) >> shift)
}

/// Decode an unsigned value from little-endian bytes of any width up to 16.
fn read_unsigned(bytes: &[u8]) -> Result<u128, FfiError> {
    let mut buffer = [0_u8; 16];
    if bytes.len() > buffer.len() {
        return Err(FfiError::UnsupportedType("integer"));
    }
    for (destination, source) in buffer.iter_mut().zip(bytes) {
        *destination = *source;
    }
    Ok(u128::from_le_bytes(buffer))
}

/// Read the first byte of a scalar result.
fn read_byte(bytes: &[u8]) -> Result<u8, FfiError> {
    bytes.first().copied().ok_or(FfiError::ValueOutOfRange {
        type_name: "boolean",
    })
}

/// Decode a Lisp boolean from a foreign argument.
fn boolean_value(value: Word) -> Result<bool, FfiError> {
    if value == Word::NIL {
        return Ok(false);
    }
    if value == Word::TRUE {
        return Ok(true);
    }
    match value.as_fixnum() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(FfiError::TypeMismatch {
            type_name: "boolean",
        }),
    }
}

/// Decode a Lisp character into its Unicode scalar value.
///
/// `Word::character` uses the character lowtag. A cons address is a heap
/// pointer far above the Unicode scalar range, so the scalar bound separates
/// the two; this mirrors `ncl-printer`'s `character_code`.
fn character_value(value: Word) -> Result<u32, FfiError> {
    const SCALAR_LIMIT: u64 = 1 << 25;
    if value.lowtag() == LOWTAG_LIST && value != Word::NIL && value.bits() < SCALAR_LIMIT {
        u32::try_from(value.bits() >> 4).map_err(|_| FfiError::TypeMismatch { type_name: "char" })
    } else {
        Err(FfiError::TypeMismatch { type_name: "char" })
    }
}

/// Decode a Lisp value into an `f32`, accepting a single- or double-float.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a double-float passed where a float is declared narrows by design"
)]
fn single_value(ctx: &ThreadContext, value: Word) -> Result<f32, FfiError> {
    if value.lowtag() == LOWTAG_SINGLE_FLOAT {
        let bits = (value.bits() >> 4) & 0xFFFF_FFFF;
        let bits = u32::try_from(bits).map_err(|_| FfiError::TypeMismatch {
            type_name: "single-float",
        })?;
        return Ok(f32::from_bits(bits));
    }
    double_value(ctx, DoubleFloat::from_word(value))
        .map(f64_to_f32)
        .map_err(|_| FfiError::TypeMismatch {
            type_name: "single-float",
        })
}

/// Decode a Lisp value into an `f64`, accepting a single- or double-float.
fn double_value_of(ctx: &ThreadContext, value: Word) -> Result<f64, FfiError> {
    if value.lowtag() == LOWTAG_SINGLE_FLOAT {
        let bits = (value.bits() >> 4) & 0xFFFF_FFFF;
        let bits = u32::try_from(bits).map_err(|_| FfiError::TypeMismatch {
            type_name: "double-float",
        })?;
        return Ok(f64::from(f32::from_bits(bits)));
    }
    double_value(ctx, DoubleFloat::from_word(value)).map_err(|_| FfiError::TypeMismatch {
        type_name: "double-float",
    })
}

const LOWTAG_LIST: u8 = 1;
const LOWTAG_SINGLE_FLOAT: u8 = 2;

#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "the foreign single-float ABI explicitly narrows a double value"
)]
const fn f64_to_f32(value: f64) -> f32 {
    value as f32
}

/// Decode a pointer argument into an address.
fn pointer_address(value: Word) -> Result<usize, FfiError> {
    if value == Word::NIL {
        return Ok(0);
    }
    value.as_fixnum().map_or(
        Err(FfiError::TypeMismatch {
            type_name: "pointer",
        }),
        |fixnum| {
            usize::try_from(fixnum).map_err(|_| FfiError::ValueOutOfRange {
                type_name: "pointer",
            })
        },
    )
}

/// Decode any Lisp integer into an `i128`.
fn word_to_i128(ctx: &ThreadContext, value: Word) -> Result<i128, FfiError> {
    if let Some(fixnum) = value.as_fixnum() {
        return Ok(i128::from(fixnum));
    }
    let (negative, magnitude) = word_magnitude(ctx, value)?;
    if negative {
        let magnitude = i128::try_from(magnitude).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        })?;
        Ok(-magnitude)
    } else {
        i128::try_from(magnitude).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        })
    }
}

/// Decode a non-negative Lisp integer into a `u128`.
fn word_to_u128(ctx: &ThreadContext, value: Word) -> Result<u128, FfiError> {
    if let Some(fixnum) = value.as_fixnum() {
        return u128::try_from(fixnum).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        });
    }
    let (negative, magnitude) = word_magnitude(ctx, value)?;
    if negative {
        return Err(FfiError::ValueOutOfRange {
            type_name: "integer",
        });
    }
    Ok(magnitude)
}

/// Decode a bignum into its sign and magnitude.
fn word_magnitude(ctx: &ThreadContext, value: Word) -> Result<(bool, u128), FfiError> {
    let bignum = Bignum::from_word(value);
    let limbs = bignum_limbs(ctx, bignum).map_err(|_| FfiError::TypeMismatch {
        type_name: "integer",
    })?;
    let negative = bignum_sign(ctx, bignum).map_err(|_| FfiError::TypeMismatch {
        type_name: "integer",
    })?;
    let mut magnitude: u128 = 0;
    let mut scale: u128 = 1;
    for limb in &limbs {
        let contribution =
            u128::from(*limb)
                .checked_mul(scale)
                .ok_or(FfiError::ValueOutOfRange {
                    type_name: "integer",
                })?;
        magnitude = magnitude
            .checked_add(contribution)
            .ok_or(FfiError::ValueOutOfRange {
                type_name: "integer",
            })?;
        scale = scale
            .checked_mul(1_u128 << 32)
            .ok_or(FfiError::ValueOutOfRange {
                type_name: "integer",
            })?;
    }
    Ok((negative, magnitude))
}

/// Build a Lisp integer from an `i128`, using a bignum outside fixnum range.
fn i128_to_word(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Result<Word, FfiError> {
    if (MOST_NEGATIVE_FIXNUM..=MOST_POSITIVE_FIXNUM).contains(&value) {
        let fixnum = i64::try_from(value).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        })?;
        Ok(Word::fixnum(fixnum))
    } else {
        Ok(make_bignum_from_i128(ctx, runtime, value)?.as_word())
    }
}

/// Build a Lisp integer from a `u128`, using a bignum outside fixnum range.
fn u128_to_word(ctx: &mut ThreadContext, runtime: &Runtime, value: u128) -> Result<Word, FfiError> {
    if value <= MOST_POSITIVE_FIXNUM_UNSIGNED {
        let fixnum = i64::try_from(value).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        })?;
        Ok(Word::fixnum(fixnum))
    } else {
        let signed = i128::try_from(value).map_err(|_| FfiError::ValueOutOfRange {
            type_name: "integer",
        })?;
        Ok(make_bignum_from_i128(ctx, runtime, signed)?.as_word())
    }
}

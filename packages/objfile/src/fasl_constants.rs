use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{SectionReader, SectionWriter};
use crate::fasl_symbols::{FaslSymbol, SymbolBindingKind};

const MAX_RECORDS: usize = 1_048_576;
const MAX_STRING_BYTES: usize = 1_048_576;
const MAX_SECTION_BYTES: usize = 256 * 1024 * 1024;
const FIXNUM_MIN: i64 = i64::MIN >> 1;
const FIXNUM_MAX: i64 = i64::MAX >> 1;

/// A constant stored in one code object's constant vector.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FaslConstant {
    /// The Lisp NIL object.
    Nil,
    /// The Lisp T object.
    Truth,
    /// A signed fixnum in the target's immediate range.
    Fixnum(
        /// Signed fixnum value.
        i64,
    ),
    /// A Unicode scalar value.
    Character(
        /// Unicode scalar.
        char,
    ),
    /// IEEE-754 binary32 bits.
    SingleFloat(
        /// Raw binary32 bits.
        u32,
    ),
    /// IEEE-754 binary64 bits.
    DoubleFloat(
        /// Raw binary64 bits.
        u64,
    ),
    /// UTF-8 string contents.
    String(
        /// String value.
        String,
    ),
    /// An identifier in the symbol table.
    SymbolReference(
        /// Symbol-table ID.
        u32,
    ),
    /// References to the car and cdr constants in this vector.
    Cons {
        /// Car constant index.
        car: u32,
        /// Cdr constant index.
        cdr: u32,
    },
    /// Constant-vector indices for a simple vector.
    SimpleVector(
        /// Element constant indices.
        Vec<u32>,
    ),
    /// A sign and little-endian magnitude limbs.
    Bignum {
        /// Whether the magnitude is negative.
        negative: bool,
        /// Little-endian magnitude limbs.
        limbs: Vec<u32>,
    },
    /// References to numerator and denominator constants in this vector.
    Ratio {
        /// Numerator constant index.
        numerator: u32,
        /// Denominator constant index.
        denominator: u32,
    },
    /// References to real and imaginary constants in this vector.
    Complex {
        /// Real-part constant index.
        real: u32,
        /// Imaginary-part constant index.
        imaginary: u32,
    },
}

/// The constants belonging to one code object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslConstantGroup {
    /// Owning code-object ID.
    pub code_object_id: u32,
    /// Constants in stable vector-index order.
    pub constants: Vec<FaslConstant>,
}

pub(super) fn encode(groups: &[FaslConstantGroup]) -> Result<Vec<u8>, FaslError> {
    check_record_count(groups.len())?;
    let mut total_constants = 0_usize;
    let mut section_size = 4_usize;
    for (index, group) in groups.iter().enumerate() {
        if usize::try_from(group.code_object_id).ok() != Some(index) {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        }
        check_record_count(group.constants.len())?;
        total_constants = total_constants
            .checked_add(group.constants.len())
            .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        if total_constants > MAX_RECORDS {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
        section_size = checked_section_growth(section_size, 8)?;
        for constant in &group.constants {
            validate_constant(constant)?;
            let (_, payload_length) = tag_and_payload_length(constant)?;
            let record_length = usize::try_from(payload_length)
                .ok()
                .and_then(|length| length.checked_add(8))
                .and_then(|length| length.checked_add(3))
                .map(|length| length & !3)
                .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
            section_size = checked_section_growth(section_size, record_length)?;
        }
    }

    let group_count = u32::try_from(groups.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    let mut output = SectionWriter::new();
    output.put_u32(group_count);
    for group in groups {
        output.put_u32(group.code_object_id);
        output.put_u32(
            u32::try_from(group.constants.len())
                .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
        );
        for constant in &group.constants {
            encode_constant(constant, &mut output)?;
            output.zero_padding(4);
        }
    }
    Ok(output.into_bytes())
}

pub(super) fn decode(
    bytes: &[u8],
    base_offset: usize,
) -> Result<Vec<FaslConstantGroup>, FaslError> {
    if bytes.len() > MAX_SECTION_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            base_offset,
        ));
    }
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidCodeObject);
    let group_count = reader.count(MAX_RECORDS, 8)?;
    let mut groups = Vec::with_capacity(group_count);
    let mut total_constants = 0_usize;
    for expected_id in 0..group_count {
        let group_offset = reader.absolute_position();
        let code_object_id = reader.u32()?;
        if usize::try_from(code_object_id).ok() != Some(expected_id) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                group_offset,
            ));
        }
        let count_offset = reader.absolute_position();
        let constant_count = reader.count(MAX_RECORDS, 8)?;
        total_constants = total_constants
            .checked_add(constant_count)
            .ok_or_else(|| FaslError::at(FaslRejectReason::ResourceLimitExceeded, count_offset))?;
        if total_constants > MAX_RECORDS {
            return Err(FaslError::at(
                FaslRejectReason::ResourceLimitExceeded,
                count_offset,
            ));
        }
        let mut constants = Vec::with_capacity(constant_count);
        for _ in 0..constant_count {
            let constant = decode_constant(&mut reader)?;
            reader.zero_padding(4)?;
            constants.push(constant);
        }
        groups.push(FaslConstantGroup {
            code_object_id,
            constants,
        });
    }
    reader.finish()?;
    Ok(groups)
}

/// Checks symbol and intra-object constant references after all sections are parsed.
pub(super) fn validate_references(
    groups: &[FaslConstantGroup],
    symbols: &[FaslSymbol],
) -> Result<(), FaslError> {
    for group in groups {
        let count = u32::try_from(group.constants.len())
            .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        for constant in &group.constants {
            match constant {
                FaslConstant::SymbolReference(symbol_id) => {
                    let symbol = usize::try_from(*symbol_id)
                        .ok()
                        .and_then(|id| symbols.get(id));
                    if symbol.is_none_or(|symbol| {
                        symbol.id != *symbol_id || symbol.binding_kind != SymbolBindingKind::Lisp
                    }) {
                        return Err(FaslError::new(FaslRejectReason::InvalidReference));
                    }
                }
                FaslConstant::Cons { car, cdr }
                | FaslConstant::Ratio {
                    numerator: car,
                    denominator: cdr,
                }
                | FaslConstant::Complex {
                    real: car,
                    imaginary: cdr,
                } if *car >= count || *cdr >= count => {
                    return Err(FaslError::new(FaslRejectReason::InvalidReference));
                }
                FaslConstant::SimpleVector(indices)
                    if indices.iter().any(|index| *index >= count) =>
                {
                    return Err(FaslError::new(FaslRejectReason::InvalidReference));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn checked_section_growth(current: usize, added: usize) -> Result<usize, FaslError> {
    let size = current
        .checked_add(added)
        .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    if size > MAX_SECTION_BYTES {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(size)
}

fn check_record_count(count: usize) -> Result<(), FaslError> {
    if count > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

fn validate_constant(constant: &FaslConstant) -> Result<(), FaslError> {
    match constant {
        FaslConstant::Fixnum(value) if !(FIXNUM_MIN..=FIXNUM_MAX).contains(value) => {
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        }
        FaslConstant::String(value) if value.len() > MAX_STRING_BYTES => {
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        }
        FaslConstant::Bignum { limbs, .. } if limbs.is_empty() || limbs.last() == Some(&0) => {
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        }
        FaslConstant::Bignum { limbs, .. } if limbs.len() > MAX_RECORDS => {
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        }
        FaslConstant::SimpleVector(indices) if indices.len() > MAX_RECORDS => {
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        }
        _ => Ok(()),
    }
}

fn encode_constant(constant: &FaslConstant, output: &mut SectionWriter) -> Result<(), FaslError> {
    let (tag, payload_length) = tag_and_payload_length(constant)?;
    output.put_u16(tag);
    output.put_u16(0);
    output.put_u32(payload_length);
    match constant {
        FaslConstant::Nil | FaslConstant::Truth => {}
        FaslConstant::Fixnum(value) => output.put_i64(*value),
        FaslConstant::Character(value) => output.put_u32(u32::from(*value)),
        FaslConstant::SingleFloat(bits) => output.put_u32(*bits),
        FaslConstant::DoubleFloat(bits) => output.put_u64(*bits),
        FaslConstant::String(value) => output.put_bytes(value.as_bytes()),
        FaslConstant::SymbolReference(id) => output.put_u32(*id),
        FaslConstant::Cons { car, cdr } => {
            output.put_u32(*car);
            output.put_u32(*cdr);
        }
        FaslConstant::SimpleVector(indices) => {
            output.put_u32(
                u32::try_from(indices.len())
                    .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
            );
            for index in indices {
                output.put_u32(*index);
            }
        }
        FaslConstant::Bignum { negative, limbs } => {
            output.put_u8(u8::from(*negative));
            output.put_bytes(&[0; 3]);
            output.put_u32(
                u32::try_from(limbs.len())
                    .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
            );
            for limb in limbs {
                output.put_u32(*limb);
            }
        }
        FaslConstant::Ratio {
            numerator,
            denominator,
        } => {
            output.put_u32(*numerator);
            output.put_u32(*denominator);
        }
        FaslConstant::Complex { real, imaginary } => {
            output.put_u32(*real);
            output.put_u32(*imaginary);
        }
    }
    Ok(())
}

fn tag_and_payload_length(constant: &FaslConstant) -> Result<(u16, u32), FaslError> {
    let (tag, length) = match constant {
        FaslConstant::Nil => (0, 0),
        FaslConstant::Truth => (1, 0),
        FaslConstant::Fixnum(_) => (2, 8),
        FaslConstant::Character(_) => (3, 4),
        FaslConstant::SingleFloat(_) => (4, 4),
        FaslConstant::DoubleFloat(_) => (5, 8),
        FaslConstant::String(value) => (6, value.len()),
        FaslConstant::SymbolReference(_) => (7, 4),
        FaslConstant::Cons { .. } => (8, 8),
        FaslConstant::SimpleVector(indices) => (
            9,
            indices
                .len()
                .checked_mul(4)
                .and_then(|length| length.checked_add(4))
                .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
        ),
        FaslConstant::Bignum { limbs, .. } => (
            10,
            limbs
                .len()
                .checked_mul(4)
                .and_then(|length| length.checked_add(8))
                .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
        ),
        FaslConstant::Ratio { .. } => (11, 8),
        FaslConstant::Complex { .. } => (12, 8),
    };
    let length = u32::try_from(length)
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    Ok((tag, length))
}

fn decode_constant(reader: &mut SectionReader<'_>) -> Result<FaslConstant, FaslError> {
    let record_offset = reader.absolute_position();
    let tag = reader.u16()?;
    let flags_offset = reader.absolute_position();
    if reader.u16()? != 0 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidCodeObject,
            flags_offset,
        ));
    }
    let length_offset = reader.absolute_position();
    let payload_length = usize::try_from(reader.u32()?)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, length_offset))?;
    let payload_offset = reader.absolute_position();
    let payload = reader.take(payload_length)?;
    let mut data = SectionReader::new(payload, payload_offset, FaslRejectReason::InvalidCodeObject);
    let constant = match tag {
        0 => FaslConstant::Nil,
        1 => FaslConstant::Truth,
        2 => {
            let value = data.i64()?;
            if !(FIXNUM_MIN..=FIXNUM_MAX).contains(&value) {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    payload_offset,
                ));
            }
            FaslConstant::Fixnum(value)
        }
        3 => {
            let value = data.u32()?;
            let character = char::from_u32(value).ok_or_else(|| {
                FaslError::at(FaslRejectReason::InvalidCodeObject, payload_offset)
            })?;
            FaslConstant::Character(character)
        }
        4 => FaslConstant::SingleFloat(data.u32()?),
        5 => FaslConstant::DoubleFloat(data.u64()?),
        6 => {
            if payload_length > MAX_STRING_BYTES {
                return Err(FaslError::at(
                    FaslRejectReason::ResourceLimitExceeded,
                    length_offset,
                ));
            }
            let value_bytes = data.take(data.remaining())?;
            let value = core::str::from_utf8(value_bytes).map_err(|error| {
                FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    payload_offset.saturating_add(error.valid_up_to()),
                )
            })?;
            FaslConstant::String(value.to_owned())
        }
        7 => FaslConstant::SymbolReference(data.u32()?),
        8 => FaslConstant::Cons {
            car: data.u32()?,
            cdr: data.u32()?,
        },
        9 => {
            let count = data.count(MAX_RECORDS, 4)?;
            let mut indices = Vec::with_capacity(count);
            for _ in 0..count {
                indices.push(data.u32()?);
            }
            FaslConstant::SimpleVector(indices)
        }
        10 => {
            let sign_offset = data.absolute_position();
            let sign = data.u8()?;
            if sign > 1 {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    sign_offset,
                ));
            }
            let reserved_offset = data.absolute_position();
            if data.take(3)?.iter().any(|byte| *byte != 0) {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    reserved_offset,
                ));
            }
            let count_offset = data.absolute_position();
            let count = data.count(MAX_RECORDS, 4)?;
            if count == 0 {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    count_offset,
                ));
            }
            let mut limbs = Vec::with_capacity(count);
            for _ in 0..count {
                limbs.push(data.u32()?);
            }
            if limbs.last() == Some(&0) {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidCodeObject,
                    count_offset,
                ));
            }
            FaslConstant::Bignum {
                negative: sign == 1,
                limbs,
            }
        }
        11 => FaslConstant::Ratio {
            numerator: data.u32()?,
            denominator: data.u32()?,
        },
        12 => FaslConstant::Complex {
            real: data.u32()?,
            imaginary: data.u32()?,
        },
        _ => {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                record_offset,
            ));
        }
    };
    data.finish()?;
    Ok(constant)
}

#[cfg(test)]
#[path = "fasl_constants_tests.rs"]
mod tests;

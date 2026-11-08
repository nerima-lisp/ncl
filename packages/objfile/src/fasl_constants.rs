use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{SectionReader, SectionWriter};

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

pub fn encode(groups: &[FaslConstantGroup]) -> Result<Vec<u8>, FaslError> {
    check_record_count(groups.len())?;
    let mut total_constants = 0_usize;
    let mut section_size = 4_usize;
    for (index, group) in groups.iter().enumerate() {
        if group.code_object_id as usize != index {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        }
        check_record_count(group.constants.len())?;
        total_constants += group.constants.len();
        if total_constants > MAX_RECORDS {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
        section_size = checked_section_growth(section_size, 8)?;
        for constant in &group.constants {
            validate_constant(constant)?;
            let (_, payload_length) = tag_and_payload_length(constant)?;
            let record_length = (payload_length as usize + 11) & !3;
            section_size = checked_section_growth(section_size, record_length)?;
        }
    }

    let group_count = groups.len() as u32;
    let mut output = SectionWriter::new();
    output.put_u32(group_count);
    for group in groups {
        output.put_u32(group.code_object_id);
        output.put_u32(group.constants.len() as u32);
        for constant in &group.constants {
            encode_constant(constant, &mut output)?;
            output.zero_padding(4);
        }
    }
    Ok(output.into_bytes())
}

pub fn decode(bytes: &[u8], base_offset: usize) -> Result<Vec<FaslConstantGroup>, FaslError> {
    if section_size_exceeds_limit(bytes.len()) {
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
        if code_object_id as usize != expected_id {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                group_offset,
            ));
        }
        let count_offset = reader.absolute_position();
        let constant_count = reader.count(MAX_RECORDS, 8)?;
        total_constants += constant_count;
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

fn checked_section_growth(current: usize, added: usize) -> Result<usize, FaslError> {
    if current > MAX_SECTION_BYTES || added > MAX_SECTION_BYTES - current {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(current + added)
}

const fn section_size_exceeds_limit(size: usize) -> bool {
    size > MAX_SECTION_BYTES
}

const fn check_record_count(count: usize) -> Result<(), FaslError> {
    if count > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

struct ConstantPayload<'a> {
    tag: u16,
    bytes: &'a [u8],
    record_offset: usize,
    length_offset: usize,
    payload_offset: usize,
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
        FaslConstant::Nil
        | FaslConstant::Truth
        | FaslConstant::Fixnum(_)
        | FaslConstant::Character(_)
        | FaslConstant::SingleFloat(_)
        | FaslConstant::DoubleFloat(_)
        | FaslConstant::String(_)
        | FaslConstant::SymbolReference(_)
        | FaslConstant::Cons { .. }
        | FaslConstant::SimpleVector(_)
        | FaslConstant::Bignum { .. }
        | FaslConstant::Ratio { .. }
        | FaslConstant::Complex { .. } => Ok(()),
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
            output.put_u32(indices.len() as u32);
            for index in indices {
                output.put_u32(*index);
            }
        }
        FaslConstant::Bignum { negative, limbs } => {
            output.put_u8(u8::from(*negative));
            output.put_bytes(&[0; 3]);
            output.put_u32(limbs.len() as u32);
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
        FaslConstant::SimpleVector(indices) => (9, indices.len() * 4 + 4),
        FaslConstant::Bignum { limbs, .. } => (10, limbs.len() * 4 + 8),
        FaslConstant::Ratio { .. } => (11, 8),
        FaslConstant::Complex { .. } => (12, 8),
    };
    let length = length as u32;
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
    let payload_length = reader.u32()? as usize;
    let payload_offset = reader.absolute_position();
    let payload = reader.take(payload_length)?;
    decode_constant_payload(&ConstantPayload {
        tag,
        bytes: payload,
        record_offset,
        length_offset,
        payload_offset,
    })
}

fn decode_constant_payload(record: &ConstantPayload<'_>) -> Result<FaslConstant, FaslError> {
    let tag = record.tag;
    let bytes = record.bytes;
    let record_offset = record.record_offset;
    let length_offset = record.length_offset;
    let payload_offset = record.payload_offset;
    let mut data = SectionReader::new(bytes, payload_offset, FaslRejectReason::InvalidCodeObject);
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
            if bytes.len() > MAX_STRING_BYTES {
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
        10 => decode_bignum(&mut data)?,
        11 => FaslConstant::Ratio {
            numerator: data.u32()?,
            denominator: data.u32()?,
        },
        12 => FaslConstant::Complex {
            real: data.u32()?,
            imaginary: data.u32()?,
        },
        13..=u16::MAX => {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                record_offset,
            ));
        }
    };
    data.finish()?;
    Ok(constant)
}

fn decode_bignum(data: &mut SectionReader<'_>) -> Result<FaslConstant, FaslError> {
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
    Ok(FaslConstant::Bignum {
        negative: sign == 1,
        limbs,
    })
}

#[cfg(test)]
#[path = "fasl_constants_tests.rs"]
mod tests;

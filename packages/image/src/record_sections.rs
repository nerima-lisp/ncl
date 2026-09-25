use crate::error::ImageError;
use crate::format::{narrow, put_string, put_u8, put_u32, put_u64, Reader};

use super::record_header::*;
use super::record_refs::{get_ref, get_refs, put_ref, put_refs};
use super::record_types::Record;

/// Encode one object record.
pub fn put_record(out: &mut Vec<u8>, record: &Record) -> Result<(), ImageError> {
    match record {
        Record::Cons { car, cdr } => {
            put_u8(out, KIND_CONS);
            put_ref(out, *car);
            put_ref(out, *cdr);
        }
        Record::Symbol {
            package,
            name,
            flags,
            value,
            function,
            plist,
        } => {
            put_u8(out, KIND_SYMBOL);
            put_string(out, package)?;
            put_string(out, name)?;
            put_u32(out, *flags);
            put_ref(out, *value);
            put_ref(out, *function);
            put_ref(out, *plist);
        }
        Record::Package { name, nicknames } => {
            put_u8(out, KIND_PACKAGE);
            put_string(out, name)?;
            put_u32(out, narrow(nicknames.len(), "nickname count")?);
            for nickname in nicknames {
                put_string(out, nickname)?;
            }
        }
        Record::String(value) => {
            put_u8(out, KIND_STRING);
            put_string(out, value)?;
        }
        Record::Vector(elements) => {
            put_u8(out, KIND_VECTOR);
            put_refs(out, elements)?;
        }
        Record::SpecializedArray {
            element_type,
            elements,
        } => {
            put_u8(out, KIND_SPECIALIZED_ARRAY);
            put_u8(out, *element_type);
            put_u32(out, narrow(elements.len(), "element count")?);
            for element in elements {
                put_u64(out, *element);
            }
        }
        Record::HashTable {
            test,
            weakness,
            entries,
        } => {
            put_u8(out, KIND_HASH_TABLE);
            put_u8(out, *test);
            put_u8(out, *weakness);
            put_u32(out, narrow(entries.len(), "entry count")?);
            for (key, value) in entries {
                put_ref(out, *key);
                put_ref(out, *value);
            }
        }
        Record::Structure { slots } => {
            put_u8(out, KIND_STRUCTURE);
            put_refs(out, slots)?;
        }
        Record::Instance { class, slots } => {
            put_u8(out, KIND_INSTANCE);
            put_ref(out, *class);
            put_refs(out, slots)?;
        }
        Record::Function {
            closure,
            entry,
            name,
            lambda_list,
            code,
            captures,
        } => {
            put_u8(out, KIND_FUNCTION);
            put_u8(out, u8::from(*closure));
            put_u64(out, *entry);
            put_ref(out, *name);
            put_ref(out, *lambda_list);
            put_ref(out, *code);
            put_refs(out, captures)?;
        }
        Record::CodeObject {
            entry,
            size,
            constants,
            stack_map,
            debug,
        } => {
            put_u8(out, KIND_CODE_OBJECT);
            put_u64(out, *entry);
            put_u64(out, *size);
            put_ref(out, *constants);
            put_ref(out, *stack_map);
            put_ref(out, *debug);
        }
        Record::Bignum { negative, limbs } => {
            put_u8(out, KIND_BIGNUM);
            put_u8(out, u8::from(*negative));
            put_u32(out, narrow(limbs.len(), "limb count")?);
            for limb in limbs {
                put_u32(out, *limb);
            }
        }
        Record::Ratio {
            numerator,
            denominator,
        } => {
            put_u8(out, KIND_RATIO);
            put_ref(out, *numerator);
            put_ref(out, *denominator);
        }
        Record::DoubleFloat { bits } => {
            put_u8(out, KIND_DOUBLE_FLOAT);
            put_u64(out, *bits);
        }
        Record::Complex { real, imag } => {
            put_u8(out, KIND_COMPLEX);
            put_ref(out, *real);
            put_ref(out, *imag);
        }
    }
    Ok(())
}

/// Decode one object record.
pub fn get_record(reader: &mut Reader<'_>) -> Result<Record, ImageError> {
    let kind = reader.u8()?;
    Ok(match kind {
        KIND_CONS => Record::Cons {
            car: get_ref(reader)?,
            cdr: get_ref(reader)?,
        },
        KIND_SYMBOL => Record::Symbol {
            package: reader.string()?,
            name: reader.string()?,
            flags: reader.u32()?,
            value: get_ref(reader)?,
            function: get_ref(reader)?,
            plist: get_ref(reader)?,
        },
        KIND_PACKAGE => {
            let name = reader.string()?;
            let count = reader.u32()? as usize;
            let mut nicknames = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                nicknames.push(reader.string()?);
            }
            Record::Package { name, nicknames }
        }
        KIND_STRING => Record::String(reader.string()?),
        KIND_VECTOR => Record::Vector(get_refs(reader)?),
        KIND_SPECIALIZED_ARRAY => {
            let element_type = reader.u8()?;
            let count = reader.u32()? as usize;
            let mut elements = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                elements.push(reader.u64()?);
            }
            Record::SpecializedArray {
                element_type,
                elements,
            }
        }
        KIND_HASH_TABLE => {
            let test = reader.u8()?;
            let weakness = reader.u8()?;
            let count = reader.u32()? as usize;
            let mut entries = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                entries.push((get_ref(reader)?, get_ref(reader)?));
            }
            Record::HashTable {
                test,
                weakness,
                entries,
            }
        }
        KIND_STRUCTURE => Record::Structure {
            slots: get_refs(reader)?,
        },
        KIND_INSTANCE => Record::Instance {
            class: get_ref(reader)?,
            slots: get_refs(reader)?,
        },
        KIND_FUNCTION => {
            let closure = reader.u8()? != 0;
            let entry = reader.u64()?;
            let name = get_ref(reader)?;
            let lambda_list = get_ref(reader)?;
            let code = get_ref(reader)?;
            let captures = get_refs(reader)?;
            Record::Function {
                closure,
                entry,
                name,
                lambda_list,
                code,
                captures,
            }
        }
        KIND_CODE_OBJECT => Record::CodeObject {
            entry: reader.u64()?,
            size: reader.u64()?,
            constants: get_ref(reader)?,
            stack_map: get_ref(reader)?,
            debug: get_ref(reader)?,
        },
        KIND_BIGNUM => {
            let negative = reader.u8()? != 0;
            let count = reader.u32()? as usize;
            let mut limbs = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                limbs.push(reader.u32()?);
            }
            Record::Bignum { negative, limbs }
        }
        KIND_RATIO => Record::Ratio {
            numerator: get_ref(reader)?,
            denominator: get_ref(reader)?,
        },
        KIND_DOUBLE_FLOAT => Record::DoubleFloat {
            bits: reader.u64()?,
        },
        KIND_COMPLEX => Record::Complex {
            real: get_ref(reader)?,
            imag: get_ref(reader)?,
        },
        tag => {
            return Err(ImageError::UnknownTag {
                space: "object kind",
                tag,
            });
        }
    })
}

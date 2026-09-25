//! Record and reference codec for the image payload.
//!
//! Every reachable heap object is encoded as one [`Record`]. References are
//! either immediate tagged words stored by raw bits or indices into the record
//! table, so the object table can be read and written in a single pass.

#![allow(
    clippy::missing_const_for_fn,
    reason = "codec validation is not const API"
)]

use crate::code::CodeImage;
use crate::error::ImageError;
use crate::format::{Reader, narrow, put_string, put_u8, put_u16, put_u32, put_u64};

const KIND_CONS: u8 = 0;
const KIND_SYMBOL: u8 = 1;
const KIND_PACKAGE: u8 = 2;
const KIND_STRING: u8 = 3;
const KIND_VECTOR: u8 = 4;
const KIND_SPECIALIZED_ARRAY: u8 = 5;
const KIND_HASH_TABLE: u8 = 6;
const KIND_STRUCTURE: u8 = 7;
const KIND_INSTANCE: u8 = 8;
const KIND_FUNCTION: u8 = 9;
const KIND_CODE_OBJECT: u8 = 10;
const KIND_BIGNUM: u8 = 11;
const KIND_RATIO: u8 = 12;
const KIND_DOUBLE_FLOAT: u8 = 13;
const KIND_COMPLEX: u8 = 14;

const REF_IMMEDIATE: u8 = 0;
const REF_OBJECT: u8 = 1;

/// A reference from one record to either an immediate word or another record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ref {
    /// A non-heap tagged word stored by raw bits.
    Immediate(u64),
    /// An index into the image's object record table.
    Object(u32),
}

/// One serialized heap object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Record {
    /// A cons cell.
    Cons {
        /// Car reference.
        car: Ref,
        /// Cdr reference.
        cdr: Ref,
    },
    /// An interned symbol addressed by its home package and name.
    Symbol {
        /// Home package name; empty for an uninterned symbol.
        package: String,
        /// Symbol name.
        name: String,
        /// Raw symbol flag bits.
        flags: u32,
        /// Value cell.
        value: Ref,
        /// Function cell.
        function: Ref,
        /// Property list.
        plist: Ref,
    },
    /// A package addressed by name.
    Package {
        /// Primary package name.
        name: String,
        /// Package nicknames.
        nicknames: Vec<String>,
    },
    /// A string.
    String(String),
    /// A simple vector.
    Vector(Vec<Ref>),
    /// A specialized array.
    SpecializedArray {
        /// Element type tag.
        element_type: u8,
        /// Raw element words.
        elements: Vec<u64>,
    },
    /// A hash table addressed by its live entries.
    HashTable {
        /// Comparison mode.
        test: u8,
        /// Weakness mode.
        weakness: u8,
        /// Live key/value pairs.
        entries: Vec<(Ref, Ref)>,
    },
    /// A structure instance.
    Structure {
        /// Structure slots.
        slots: Vec<Ref>,
    },
    /// A CLOS instance.
    Instance {
        /// Class reference.
        class: Ref,
        /// Slot vector contents.
        slots: Vec<Ref>,
    },
    /// A simple function or a closure.
    Function {
        /// Whether the object is a closure with inline captures.
        closure: bool,
        /// Raw entry address.
        entry: u64,
        /// Function name.
        name: Ref,
        /// Lambda list.
        lambda_list: Ref,
        /// Code object reference.
        code: Ref,
        /// Inline captures; empty for a simple function.
        captures: Vec<Ref>,
    },
    /// A code object descriptor.
    CodeObject {
        /// Raw entry address.
        entry: u64,
        /// Code byte size.
        size: u64,
        /// Constant table.
        constants: Ref,
        /// Stack-map index.
        stack_map: Ref,
        /// Debug table.
        debug: Ref,
    },
    /// A bignum.
    Bignum {
        /// Whether the value is negative.
        negative: bool,
        /// Little-endian 32-bit limbs.
        limbs: Vec<u32>,
    },
    /// A ratio.
    Ratio {
        /// Numerator.
        numerator: Ref,
        /// Denominator.
        denominator: Ref,
    },
    /// A double float.
    DoubleFloat {
        /// Raw IEEE-754 binary64 bits.
        bits: u64,
    },
    /// A complex number.
    Complex {
        /// Real component.
        real: Ref,
        /// Imaginary component.
        imag: Ref,
    },
}

/// Validate all references against an already decoded object-table length.
pub fn validate_record(record: &Record, count: usize) -> Result<(), ImageError> {
    let check = |reference: Ref| validate_ref(reference, count);
    match record {
        Record::Cons { car, cdr } => {
            check(*car)?;
            check(*cdr)?;
        }
        Record::Symbol {
            value,
            function,
            plist,
            ..
        } => {
            check(*value)?;
            check(*function)?;
            check(*plist)?;
        }
        Record::Vector(values) | Record::Structure { slots: values } => {
            for value in values {
                check(*value)?;
            }
        }
        Record::HashTable { entries, .. } => {
            for (key, value) in entries {
                check(*key)?;
                check(*value)?;
            }
        }
        Record::Instance { class, slots } => {
            check(*class)?;
            for value in slots {
                check(*value)?;
            }
        }
        Record::Function {
            name,
            lambda_list,
            code,
            captures,
            ..
        } => {
            check(*name)?;
            check(*lambda_list)?;
            check(*code)?;
            for value in captures {
                check(*value)?;
            }
        }
        Record::CodeObject {
            constants,
            stack_map,
            debug,
            ..
        } => {
            check(*constants)?;
            check(*stack_map)?;
            check(*debug)?;
        }
        Record::Ratio {
            numerator,
            denominator,
        } => {
            check(*numerator)?;
            check(*denominator)?;
        }
        Record::Complex { real, imag } => {
            check(*real)?;
            check(*imag)?;
        }
        Record::Package { .. }
        | Record::String(_)
        | Record::SpecializedArray { .. }
        | Record::Bignum { .. }
        | Record::DoubleFloat { .. } => {}
    }
    Ok(())
}

/// Validate one reference against an object-table length.
pub fn validate_ref(reference: Ref, count: usize) -> Result<(), ImageError> {
    match reference {
        Ref::Immediate(_) => Ok(()),
        Ref::Object(index) if (index as usize) < count => Ok(()),
        Ref::Object(_) => Err(ImageError::InvalidLayout {
            field: "object reference",
        }),
    }
}

/// Encode one reference.
pub fn put_ref(out: &mut Vec<u8>, value: Ref) {
    match value {
        Ref::Immediate(bits) => {
            put_u8(out, REF_IMMEDIATE);
            put_u64(out, bits);
        }
        Ref::Object(id) => {
            put_u8(out, REF_OBJECT);
            put_u32(out, id);
        }
    }
}

/// Encode a length-prefixed reference list.
pub fn put_refs(out: &mut Vec<u8>, values: &[Ref]) -> Result<(), ImageError> {
    put_u32(out, narrow(values.len(), "reference count")?);
    for value in values {
        put_ref(out, *value);
    }
    Ok(())
}

/// Encode one object record.
#[allow(clippy::too_many_lines, reason = "flat wire-format dispatch table")]
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

/// Encode one code blob.
pub fn put_code(out: &mut Vec<u8>, code: &CodeImage) -> Result<(), ImageError> {
    put_string(out, code.function_name())?;
    put_u32(out, narrow(code.entry_offset(), "code entry offset")?);
    put_u16(out, code.frame_words());
    put_u32(out, narrow(code.bytes().len(), "code size")?);
    out.extend_from_slice(code.bytes());
    Ok(())
}

/// Decode one code blob.
pub fn get_code(reader: &mut Reader<'_>) -> Result<CodeImage, ImageError> {
    let function_name = reader.string()?;
    let entry_offset = reader.u32()? as usize;
    let frame_words = reader.u16()?;
    let size = reader.u32()? as usize;
    let bytes = reader.take(size)?.to_vec();
    CodeImage::from_raw(bytes, entry_offset, frame_words, function_name)
}

/// Decode one reference.
pub fn get_ref(reader: &mut Reader<'_>) -> Result<Ref, ImageError> {
    match reader.u8()? {
        REF_IMMEDIATE => Ok(Ref::Immediate(reader.u64()?)),
        REF_OBJECT => Ok(Ref::Object(reader.u32()?)),
        tag => Err(ImageError::UnknownTag {
            space: "reference",
            tag,
        }),
    }
}

/// Decode a length-prefixed reference list.
pub fn get_refs(reader: &mut Reader<'_>) -> Result<Vec<Ref>, ImageError> {
    let count = reader.u32()? as usize;
    let mut values = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        values.push(get_ref(reader)?);
    }
    Ok(values)
}

/// Decode one object record.
#[allow(clippy::too_many_lines, reason = "flat wire-format dispatch table")]
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

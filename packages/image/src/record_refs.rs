use crate::error::ImageError;
use crate::format::{narrow, put_u8, put_u32, put_u64, Reader};

use super::record_header::{REF_IMMEDIATE, REF_OBJECT};

/// A reference from one record to either an immediate word or another record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ref {
    /// A non-heap tagged word stored by raw bits.
    Immediate(u64),
    /// An index into the image's object record table.
    Object(u32),
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

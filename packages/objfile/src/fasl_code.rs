use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::SectionReader;
use crate::fasl_records::SectionWriter;

const MAX_RECORDS: usize = 1_048_576;
const MAX_SECTION_BYTES: usize = 256 * 1024 * 1024;

/// One validated code-object record and its machine-code bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslCodeObject<'a> {
    /// Dense code-object identifier.
    pub id: u32,
    /// Byte offset of this object's callable entry point.
    pub entry_offset: u32,
    /// Frame size in machine words.
    pub frame_words: u16,
    /// Number of constants in the owning vector.
    pub constant_count: u32,
    /// Number of stack-map entries for this code object.
    pub stack_map_count: u32,
    /// Number of debug records for this code object.
    pub debug_record_count: u32,
    /// Machine-code bytes borrowed from the containing FASL.
    pub code: &'a [u8],
}

/// Encodes code objects in dense ID order.
#[expect(
    clippy::cast_possible_truncation,
    reason = "record and section limits are checked before values are written as u32"
)]
pub fn encode(objects: &[FaslCodeObject<'_>]) -> Result<Vec<u8>, FaslError> {
    if objects.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let mut section_size = 4_usize;
    for (index, object) in objects.iter().enumerate() {
        validate_object(object, index)?;
        // The slice length and bounded section prefix fit in `usize` on supported targets.
        let padded_end = (section_size + 28 + object.code.len() + 7) & !7;
        check_section_size(padded_end, None)?;
        section_size = padded_end;
    }

    // check-added-lines: allow(as-cast) the record count is bounded by MAX_RECORDS.
    let count = objects.len() as u32;
    let mut writer = SectionWriter::new();
    writer.put_u32(count);
    for object in objects {
        writer.put_u32(object.id);
        writer.put_u32(object.entry_offset);
        // check-added-lines: allow(as-cast) check_section_size caps code bytes below u32::MAX.
        let code_length = object.code.len() as u32;
        writer.put_u32(code_length);
        writer.put_u16(object.frame_words);
        writer.put_u16(0);
        writer.put_u32(object.constant_count);
        writer.put_u32(object.stack_map_count);
        writer.put_u32(object.debug_record_count);
        writer.put_bytes(object.code);
        writer.zero_padding(8);
    }
    Ok(writer.into_bytes())
}

/// Decodes code objects without copying machine-code bytes.
pub fn decode(bytes: &[u8], base_offset: usize) -> Result<Vec<FaslCodeObject<'_>>, FaslError> {
    check_section_size(bytes.len(), Some(base_offset))?;
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidCodeObject);
    let count_offset = reader.absolute_position();
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, count_offset))?;
    if count > MAX_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            count_offset,
        ));
    }
    if count > reader.remaining() / 36 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidCodeObject,
            count_offset,
        ));
    }

    let mut objects = Vec::with_capacity(count);
    for expected_id in 0..count {
        let record_offset = reader.absolute_position();
        let id = reader.u32()?;
        if usize::try_from(id).ok() != Some(expected_id) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                record_offset,
            ));
        }
        let entry_offset = reader.u32()?;
        let code_length_offset = reader.absolute_position();
        let code_length = usize::try_from(reader.u32()?).map_err(|_| {
            FaslError::at(FaslRejectReason::ResourceLimitExceeded, code_length_offset)
        })?;
        if code_length == 0 {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                code_length_offset,
            ));
        }
        let frame_words = reader.u16()?;
        let flags_offset = reader.absolute_position();
        if reader.u16()? != 0 {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                flags_offset,
            ));
        }
        let constant_count = reader.u32()?;
        let stack_map_count = reader.u32()?;
        let debug_record_count = reader.u32()?;
        let code = reader.take(code_length)?;
        if usize::try_from(entry_offset)
            .ok()
            .is_none_or(|entry| entry >= code.len())
        {
            return Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                record_offset.saturating_add(4),
            ));
        }
        reader.zero_padding(8)?;
        objects.push(FaslCodeObject {
            id,
            entry_offset,
            frame_words,
            constant_count,
            stack_map_count,
            debug_record_count,
            code,
        });
    }
    reader.finish()?;
    Ok(objects)
}

fn validate_object(object: &FaslCodeObject<'_>, expected_id: usize) -> Result<(), FaslError> {
    if usize::try_from(object.id).ok() != Some(expected_id)
        || object.code.is_empty()
        || usize::try_from(object.entry_offset)
            .ok()
            .is_none_or(|entry| entry >= object.code.len())
    {
        return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
    }
    Ok(())
}

fn check_section_size(size: usize, offset: Option<usize>) -> Result<(), FaslError> {
    if size <= MAX_SECTION_BYTES {
        return Ok(());
    }
    Err(offset.map_or_else(
        || FaslError::new(FaslRejectReason::ResourceLimitExceeded),
        |offset| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset),
    ))
}

#[cfg(test)]
#[path = "tests/fasl_code.rs"]
mod tests;

use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_code::FaslCodeObject;
use crate::fasl_records::{SectionReader, SectionWriter};

const MAX_RECORDS: usize = 1_048_576;

/// One ordered top-level code entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaslTopLevelRecord {
    /// Dense source-order ordinal.
    pub ordinal: u32,
    /// Referenced code-object identifier.
    pub code_object_id: u32,
    /// Entry offset, which must equal the referenced code object's entry.
    pub entry_offset: u32,
    /// Bit 0 executes during LOAD; bit 1 marks a function definition.
    pub flags: u32,
}

/// Encodes top-level records after checking their code-object entries.
pub fn encode(
    records: &[FaslTopLevelRecord],
    code_objects: &[FaslCodeObject<'_>],
) -> Result<Vec<u8>, FaslError> {
    validate(records, code_objects, None)?;
    let count = records.len() as u32;
    let mut writer = SectionWriter::new();
    writer.put_u32(count);
    for record in records {
        writer.put_u32(record.ordinal);
        writer.put_u32(record.code_object_id);
        writer.put_u32(record.entry_offset);
        writer.put_u32(record.flags);
    }
    Ok(writer.into_bytes())
}

/// Decodes fixed-width top-level records and validates their code-object entries.
pub fn decode(
    bytes: &[u8],
    base_offset: usize,
    code_objects: &[FaslCodeObject<'_>],
) -> Result<Vec<FaslTopLevelRecord>, FaslError> {
    let mut reader =
        SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidTopLevelRecord);
    let count_offset = reader.absolute_position();
    let count = reader.u32()? as usize;
    if count > MAX_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            count_offset,
        ));
    }
    if count > reader.remaining() / 16 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidTopLevelRecord,
            count_offset,
        ));
    }

    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let bytes = reader.take(16)?;
        records.push(FaslTopLevelRecord {
            ordinal: u32_at(bytes, 0),
            code_object_id: u32_at(bytes, 4),
            entry_offset: u32_at(bytes, 8),
            flags: u32_at(bytes, 12),
        });
    }
    reader.finish()?;
    validate(&records, code_objects, Some(base_offset))?;
    Ok(records)
}

fn validate(
    records: &[FaslTopLevelRecord],
    code_objects: &[FaslCodeObject<'_>],
    base_offset: Option<usize>,
) -> Result<(), FaslError> {
    if records.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    for (index, record) in records.iter().enumerate() {
        let offset = base_offset.map(|base| base.saturating_add(4 + index.saturating_mul(16)));
        let error = offset.map_or_else(
            || FaslError::new(FaslRejectReason::InvalidTopLevelRecord),
            |offset| FaslError::at(FaslRejectReason::InvalidTopLevelRecord, offset),
        );
        if record.ordinal as usize != index
            || record.flags & !3 != 0
            || (record.flags & 2 != 0 && record.flags & 1 == 0)
        {
            return Err(error);
        }
        let code_index = record.code_object_id as usize;
        let code = code_objects.get(code_index).ok_or(error)?;
        if code.id != record.code_object_id {
            return Err(error);
        }
        if code.entry_offset != record.entry_offset {
            return Err(error);
        }
    }
    Ok(())
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
#[path = "fasl_top_level_tests.rs"]
mod tests;

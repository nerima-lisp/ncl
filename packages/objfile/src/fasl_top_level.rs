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
#[expect(
    clippy::cast_possible_truncation,
    reason = "record count is checked against MAX_RECORDS before encoding"
)]
pub fn encode(
    records: &[FaslTopLevelRecord],
    code_objects: &[FaslCodeObject<'_>],
) -> Result<Vec<u8>, FaslError> {
    validate(records, code_objects, None)?;
    // check-added-lines: allow(as-cast) the record count is bounded by MAX_RECORDS.
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
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, count_offset))?;
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
        records.push(FaslTopLevelRecord {
            ordinal: reader.u32()?,
            code_object_id: reader.u32()?,
            entry_offset: reader.u32()?,
            flags: reader.u32()?,
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
        if usize::try_from(record.ordinal).ok() != Some(index)
            || record.flags & !3 != 0
            || (record.flags & 2 != 0 && record.flags & 1 == 0)
        {
            return Err(error);
        }
        let code_index = usize::try_from(record.code_object_id).map_err(|_| error)?;
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

#[cfg(test)]
#[path = "fasl_top_level_tests.rs"]
mod tests;

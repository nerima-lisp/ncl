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
pub(super) fn encode(
    records: &[FaslTopLevelRecord],
    code_objects: &[FaslCodeObject<'_>],
) -> Result<Vec<u8>, FaslError> {
    validate(records, code_objects, None)?;
    let count = u32::try_from(records.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
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
pub(super) fn decode(
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
        if code.id != record.code_object_id || code.entry_offset != record.entry_offset {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_records_have_stable_ordinals_and_bytes() {
        let code_objects = [FaslCodeObject {
            id: 0,
            entry_offset: 3,
            frame_words: 0,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code: &[0; 4],
        }];
        let records = [FaslTopLevelRecord {
            ordinal: 0,
            code_object_id: 0,
            entry_offset: 3,
            flags: 3,
        }];
        let expected = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 3, 0, 0, 0];
        assert_eq!(encode(&records, &code_objects), Ok(expected.to_vec()));
        assert_eq!(decode(&expected, 320, &code_objects), Ok(records.to_vec()));
    }

    #[test]
    fn top_level_rejects_invalid_flags_and_code_entries() {
        let code_objects = [FaslCodeObject {
            id: 0,
            entry_offset: 1,
            frame_words: 0,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code: &[0; 2],
        }];
        let valid = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mut bad_flags = valid;
        bad_flags[16] = 4;
        assert_eq!(
            decode(&bad_flags, 320, &code_objects).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );
        let mut bad_entry = valid;
        bad_entry[12] = 0;
        assert_eq!(
            decode(&bad_entry, 320, &code_objects).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );
    }
}

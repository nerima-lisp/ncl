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
mod tests {
    use super::*;

    fn code_object(id: u32, entry_offset: u32) -> FaslCodeObject<'static> {
        FaslCodeObject {
            id,
            entry_offset,
            frame_words: 0,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code: &[0; 4],
        }
    }

    fn record(
        ordinal: u32,
        code_object_id: u32,
        entry_offset: u32,
        flags: u32,
    ) -> FaslTopLevelRecord {
        FaslTopLevelRecord {
            ordinal,
            code_object_id,
            entry_offset,
            flags,
        }
    }

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
    fn top_level_records_round_trip_multiple_ordinals_and_zero_flags() {
        let code_objects = [code_object(0, 0), code_object(1, 3)];
        let records = [record(0, 0, 0, 0), record(1, 1, 3, 1)];
        let expected = [
            2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 3,
            0, 0, 0, 1, 0, 0, 0,
        ];
        assert_eq!(encode(&records, &code_objects), Ok(expected.to_vec()));
        assert_eq!(decode(&expected, 500, &code_objects), Ok(records.to_vec()));

        let mut bad_second_flags = expected;
        bad_second_flags[32..36].copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_second_flags, 500, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 520))
        );
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

    #[test]
    fn top_level_encoder_checks_ordinal_function_flag_and_code_identity() {
        let code_objects = [code_object(0, 1)];
        assert_eq!(encode(&[], &[]), Ok(vec![0, 0, 0, 0]));
        assert_eq!(decode(&[0, 0, 0, 0], 320, &[]), Ok(Vec::new()));

        assert_eq!(
            encode(&[record(1, 0, 1, 1)], &code_objects),
            Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
        );
        assert_eq!(
            encode(&[record(0, 0, 1, 2)], &code_objects),
            Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
        );
        assert_eq!(
            encode(&[record(0, 1, 1, 1)], &code_objects),
            Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
        );
        assert_eq!(
            encode(&[record(0, 0, 0, 1)], &code_objects),
            Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
        );
        assert_eq!(
            encode(&[record(0, 0, 1, 1)], &[code_object(1, 1)]),
            Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
        );

        let too_many = vec![record(0, 0, 0, 0); MAX_RECORDS + 1];
        assert_eq!(
            encode(&too_many, &[]),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );
    }

    #[test]
    fn top_level_decoder_checks_counts_ordinals_references_and_complete_sections() {
        let code_objects = [code_object(0, 1)];
        let valid = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];

        for prefix_end in 0..valid.len() {
            assert!(matches!(
                decode(&valid[..prefix_end], 320, &code_objects),
                Err(FaslError {
                    reason: FaslRejectReason::InvalidTopLevelRecord,
                    ..
                })
            ));
        }

        assert_eq!(
            decode(&[], 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 320))
        );
        assert_eq!(
            decode(&[1, 0, 0, 0], 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 320))
        );
        assert_eq!(
            decode(&0x0010_0001_u32.to_le_bytes(), 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
        );

        let mut bad_ordinal = valid;
        bad_ordinal[4..8].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_ordinal, 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );
        let mut bad_code_id = valid;
        bad_code_id[8..12].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_code_id, 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );
        assert_eq!(
            decode(&valid, 320, &[code_object(1, 1)]),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );
        let mut function_without_execute = valid;
        function_without_execute[16..20].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode(&function_without_execute, 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 324))
        );

        let mut trailing = valid.to_vec();
        trailing.push(0);
        assert_eq!(
            decode(&trailing, 320, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidTopLevelRecord, 340))
        );
    }
}

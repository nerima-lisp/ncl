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
pub fn encode(objects: &[FaslCodeObject<'_>]) -> Result<Vec<u8>, FaslError> {
    if objects.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let mut section_size = 4_usize;
    for (index, object) in objects.iter().enumerate() {
        validate_object(object, index, None)?;
        let padded_end = section_size
            .checked_add(28)
            .and_then(|size| size.checked_add(object.code.len()))
            .and_then(|size| size.checked_add(7))
            .map(|size| size & !7)
            .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        if padded_end > MAX_SECTION_BYTES {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
        section_size = padded_end;
    }

    let count = u32::try_from(objects.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    let mut writer = SectionWriter::new();
    writer.put_u32(count);
    for object in objects {
        writer.put_u32(object.id);
        writer.put_u32(object.entry_offset);
        writer.put_u32(
            u32::try_from(object.code.len())
                .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?,
        );
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
    if bytes.len() > MAX_SECTION_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            base_offset,
        ));
    }
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

fn validate_object(
    object: &FaslCodeObject<'_>,
    expected_id: usize,
    offset: Option<usize>,
) -> Result<(), FaslError> {
    let error = || {
        offset.map_or_else(
            || FaslError::new(FaslRejectReason::InvalidCodeObject),
            |offset| FaslError::at(FaslRejectReason::InvalidCodeObject, offset),
        )
    };
    if usize::try_from(object.id).ok() != Some(expected_id)
        || object.code.is_empty()
        || usize::try_from(object.entry_offset)
            .ok()
            .is_none_or(|entry| entry >= object.code.len())
    {
        return Err(error());
    }
    if u32::try_from(object.code.len()).is_err() {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    fn object(id: u32, entry_offset: u32, code: &[u8]) -> FaslCodeObject<'_> {
        FaslCodeObject {
            id,
            entry_offset,
            frame_words: 0,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code,
        }
    }

    #[test]
    fn code_section_matches_fixed_record_and_padding_bytes() {
        let code = [0xaa, 0xbb, 0xcc];
        let object = FaslCodeObject {
            id: 0,
            entry_offset: 1,
            frame_words: 2,
            constant_count: 3,
            stack_map_count: 4,
            debug_record_count: 5,
            code: &code,
        };
        let encoded = encode(core::slice::from_ref(&object));
        let mut expected = vec![1, 0, 0, 0];
        expected.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0]);
        expected.extend_from_slice(&[2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0]);
        expected.extend_from_slice(&code);
        expected.extend_from_slice(&[0; 5]);
        assert_eq!(encoded, Ok(expected.clone()));
        assert_eq!(decode(&expected, 320), Ok(vec![object]));
    }

    #[test]
    fn code_section_round_trips_multiple_dense_objects_at_aligned_boundaries() {
        let first_code = [0x90];
        let second_code = [0xaa; 8];
        let objects = [object(0, 0, &first_code), object(1, 7, &second_code)];
        let encoded = must_succeed(encode(&objects));
        assert_eq!(encoded.len(), 80);
        assert_eq!(&encoded[33..40], &[0; 7]);
        assert_eq!(&encoded[76..80], &[0; 4]);
        assert_eq!(decode(&encoded, 400), Ok(objects.to_vec()));
    }

    #[test]
    fn code_section_rejects_reserved_bits_bad_entry_and_trailing_bytes() {
        let valid = encode(&[FaslCodeObject {
            id: 0,
            entry_offset: 0,
            frame_words: 0,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code: &[0x90],
        }]);
        let valid = must_succeed(valid);
        let mut bad_flags = valid.clone();
        bad_flags[18..20].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            decode(&bad_flags, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 338))
        );
        let mut trailing = valid;
        trailing.push(0);
        assert_eq!(
            decode(&trailing, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 360))
        );
    }

    #[test]
    fn code_encoder_rejects_sparse_ids_empty_code_and_out_of_range_entries() {
        assert_eq!(
            encode(&[object(1, 0, &[0x90])]),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(
            encode(&[object(0, 0, &[])]),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(
            encode(&[object(0, 1, &[0x90])]),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(encode(&[]), Ok(vec![0, 0, 0, 0]));
        assert_eq!(decode(&[0, 0, 0, 0], 320), Ok(Vec::new()));

        let code = [0x90];
        let too_many = vec![object(1, 0, &code); MAX_RECORDS + 1];
        assert_eq!(
            encode(&too_many),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );
    }

    #[test]
    fn code_decoder_rejects_bad_ids_lengths_padding_and_record_counts() {
        let valid = must_succeed(encode(&[object(0, 0, &[0x90])]));

        assert_eq!(
            decode(&[1, 0, 0], 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 320))
        );

        let mut bad_id = valid.clone();
        bad_id[4..8].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_id, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 324))
        );

        let mut zero_length = valid.clone();
        zero_length[12..16].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            decode(&zero_length, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 332))
        );

        let mut bad_entry = valid.clone();
        bad_entry[8..12].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_entry, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 328))
        );

        let mut bad_padding = valid.clone();
        bad_padding[35] = 1;
        assert_eq!(
            decode(&bad_padding, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 355))
        );

        let mut truncated_code = valid.clone();
        truncated_code[12..16].copy_from_slice(&10_u32.to_le_bytes());
        assert_eq!(
            decode(&truncated_code, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 352))
        );

        let mut truncated_padding = valid;
        truncated_padding[12..16].copy_from_slice(&9_u32.to_le_bytes());
        truncated_padding.extend_from_slice(&[0; 7]);
        assert_eq!(
            decode(&truncated_padding, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 361))
        );
        assert_eq!(
            decode(&[1, 0, 0, 0], 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 320))
        );
        assert_eq!(
            decode(&0x0010_0001_u32.to_le_bytes(), 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
        );
    }
}

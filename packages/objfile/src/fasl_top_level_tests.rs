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

fn record(ordinal: u32, code_object_id: u32, entry_offset: u32, flags: u32) -> FaslTopLevelRecord {
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
        2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 3, 0,
        0, 0, 1, 0, 0, 0,
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

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
fn code_section_size_boundary_preserves_encoder_and_decoder_errors() {
    assert_eq!(check_section_size(MAX_SECTION_BYTES, None), Ok(()));
    assert_eq!(
        check_section_size(MAX_SECTION_BYTES + 1, None),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
    assert_eq!(check_section_size(MAX_SECTION_BYTES, Some(320)), Ok(()));
    assert_eq!(
        check_section_size(MAX_SECTION_BYTES + 1, Some(320)),
        Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
    );
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

    let mut bad_second_flags = encoded.clone();
    bad_second_flags[54..56].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode(&bad_second_flags, 400).map(|_| ()),
        Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 454))
    );
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

    for end in 0..valid.len() {
        assert!(matches!(
            decode(&valid[..end], 320),
            Err(FaslError {
                reason: FaslRejectReason::InvalidCodeObject,
                ..
            })
        ));
    }

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

#[test]
fn code_decoder_reaches_truncated_following_fixed_fields_after_code_bytes() {
    let mut bytes = must_succeed(encode(&[object(0, 0, &[0x90]), object(1, 0, &[0; 8])]));
    bytes[12..16].copy_from_slice(&44_u32.to_le_bytes());
    let second_header = bytes[40..68].to_vec();

    for (prefix, field_offset) in [
        (3, 0),
        (7, 4),
        (11, 8),
        (13, 12),
        (15, 14),
        (19, 16),
        (23, 20),
        (27, 24),
    ] {
        let mut truncated = bytes.clone();
        truncated.extend_from_slice(&second_header[..prefix]);
        assert_eq!(
            decode(&truncated, 320),
            Err(FaslError::at(
                FaslRejectReason::InvalidCodeObject,
                400 + field_offset
            )),
            "second record prefix length {prefix}"
        );
    }
}

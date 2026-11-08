use super::*;

const CODE_8: [u8; 8] = [0; 8];
const CODE_16: [u8; 16] = [0; 16];

fn entry() -> SafepointEntry {
    SafepointEntry {
        pc_offset: 7,
        frame_words: 8,
        slot_words: 5,
        word_slot_count: 2,
        register_mask: 0b1001,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: vec![0, 3],
    }
}

fn must_encode(groups: &[SafepointGroup]) -> Vec<u8> {
    encode_section(groups).unwrap_or_else(|error| panic!("valid stack map rejected: {error}"))
}

fn group(entries: Vec<SafepointEntry>) -> SafepointGroup {
    SafepointGroup {
        code_object_id: 0,
        entries,
    }
}

fn layout(code: &'static [u8], frame_words: u16) -> [FaslCodeLayout<'static>; 1] {
    [FaslCodeLayout {
        id: 0,
        entry_offset: 0,
        code,
        frame_words,
        constant_count: 0,
    }]
}

fn assert_bad(bytes: &[u8], base: usize, layout: &[FaslCodeLayout<'_>], offset: usize) {
    assert_eq!(
        decode_section(bytes, base, layout),
        Err(FaslError::at(FaslRejectReason::InvalidSafepointMap, offset))
    );
}

#[test]
fn safepoint_header_matches_the_existing_sixteen_byte_layout() {
    let bytes = encode_section(&[group(vec![entry()])]);
    assert_eq!(
        bytes,
        Ok(vec![
            1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 8, 0, 5, 0, 2, 0, 9, 0, 0, 0, 0, 0, 4,
            0, 0, 3, 0,
        ])
    );
    let eight_slots = SafepointEntry {
        slot_words: 8,
        word_slot_count: 1,
        register_mask: 0,
        slot_bitmap: vec![4],
        register_ids: vec![],
        ..entry()
    };
    let groups = [group(vec![eight_slots])];
    let layout = layout(&CODE_8, 8);
    assert_eq!(
        decode_section(&must_encode(&groups), 0, &layout),
        Ok(groups.to_vec())
    );
}

#[test]
fn safepoint_decoder_rejects_bad_group_and_entry_counts() {
    let valid = must_encode(&[group(vec![entry()])]);
    let layout = layout(&CODE_16, 8);
    for (range, value, offset) in [(0..4, 0_u32, 500), (4..8, 1, 504), (8..12, 2, 508)] {
        let mut bytes = valid.clone();
        bytes[range].copy_from_slice(&value.to_le_bytes());
        assert_bad(&bytes, 500, &layout, offset);
    }
    let mut invalid_wire_id = valid.clone();
    invalid_wire_id[4..8].copy_from_slice(&1_u32.to_le_bytes());
    assert_bad(&invalid_wire_id, 500, &layout, 504);
    assert_bad(&valid, 500, &[FaslCodeLayout { id: 1, ..layout[0] }], 504);
    for (range, offset) in [(0..4, 500), (8..12, 508)] {
        let mut excessive = valid.clone();
        excessive[range].copy_from_slice(&1_048_577_u32.to_le_bytes());
        assert_eq!(
            decode_section(&excessive, 500, &layout),
            Err(FaslError::at(
                FaslRejectReason::ResourceLimitExceeded,
                offset
            ))
        );
    }
}

#[test]
fn safepoint_decoder_rejects_invalid_entry_fields_and_truncation() {
    let valid = must_encode(&[group(vec![entry()])]);
    let layout = layout(&CODE_16, 8);
    for (offset, value) in [(18, 2_u16), (20, 6), (16, 7)] {
        let mut bytes = valid.clone();
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert_bad(&bytes, 500, &layout, 512);
    }
    let mut bad_pc = valid.clone();
    bad_pc[12..16].copy_from_slice(&16_u32.to_le_bytes());
    assert_bad(&bad_pc, 500, &layout, 512);
    let mut bad_bitmap = valid.clone();
    bad_bitmap[28] = 0;
    assert_bad(&bad_bitmap, 500, &layout, 528);
    let mut bad_ids = valid.clone();
    bad_ids[31..33].copy_from_slice(&2_u16.to_le_bytes());
    assert_bad(&bad_ids, 500, &layout, 529);
    let mut unused_bits = valid.clone();
    unused_bits[28] |= 0b1000_0000;
    assert_bad(&unused_bits, 500, &layout, 528);
    let mut unsorted = must_encode(&[group(vec![
        entry(),
        SafepointEntry {
            pc_offset: 8,
            ..entry()
        },
    ])]);
    unsorted[33..37].copy_from_slice(&6_u32.to_le_bytes());
    assert_bad(&unsorted, 500, &layout, 533);
    for end in 0..valid.len() {
        assert!(matches!(
            decode_section(&valid[..end], 500, &layout),
            Err(FaslError {
                reason: FaslRejectReason::InvalidSafepointMap,
                ..
            })
        ));
    }
}

#[test]
fn safepoint_encoder_rejects_duplicates_bad_ids_and_entry_shapes() {
    let invalid = Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
    let mut bad_ids = entry();
    bad_ids.register_ids = vec![0, 2];
    assert_eq!(encode_section(&[group(vec![entry(), entry()])]), invalid);
    assert_eq!(
        encode_section(&[SafepointGroup {
            code_object_id: 1,
            entries: vec![]
        }]),
        invalid
    );
    assert_eq!(encode_section(&[group(vec![]), group(vec![]),]), invalid);
    for bad in [
        bad_ids,
        SafepointEntry {
            slot_words: 2,
            ..entry()
        },
        SafepointEntry {
            word_slot_count: 6,
            ..entry()
        },
        SafepointEntry {
            slot_bitmap: vec![],
            ..entry()
        },
        SafepointEntry {
            slot_bitmap: vec![0],
            ..entry()
        },
    ] {
        assert_eq!(encode_section(&[group(vec![bad])]), invalid);
    }
    let groups = vec![group(vec![]); MAX_RECORDS + 1];
    assert_eq!(
        encode_section(&groups),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
    drop(groups);

    let empty = SafepointEntry {
        slot_bitmap: Vec::new(),
        register_ids: Vec::new(),
        ..entry()
    };
    let entries = vec![empty; MAX_RECORDS + 1];
    assert_eq!(
        validate_entries(&entries),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
}

#[test]
fn safepoint_reader_reaches_truncated_following_group_headers() {
    let groups = [
        group(vec![entry()]),
        SafepointGroup {
            code_object_id: 1,
            entries: Vec::new(),
        },
    ];
    let bytes = must_encode(&groups);
    let first_layout = layout(&CODE_16, 8)[0];
    let code_objects = [
        first_layout,
        FaslCodeLayout {
            id: 1,
            ..first_layout
        },
    ];
    let first_group_end = 4 + 8 + 21;
    for (prefix, offset) in [(0, 500 + first_group_end), (4, 500 + first_group_end + 4)] {
        assert_eq!(
            decode_section(&bytes[..first_group_end + prefix], 500, &code_objects),
            Err(FaslError::at(FaslRejectReason::InvalidSafepointMap, offset))
        );
    }
}

#[test]
fn safepoint_reader_reaches_truncated_following_entry_headers() {
    let cases = [
        (0b11_1111, vec![0, 1, 2, 3, 4, 5], 3, 0),
        (0b01_1111, vec![0, 1, 2, 3, 4], 5, 4),
        (0b00_1111, vec![0, 1, 2, 3], 7, 6),
        (0b00_0111, vec![0, 1, 2], 9, 8),
        (0b00_0011, vec![0, 1], 11, 10),
        (0, Vec::new(), 15, 12),
    ];
    let layout = layout(&CODE_16, 8);

    for (register_mask, register_ids, prefix, field_offset) in cases {
        let first_entry_length = 17 + register_ids.len() * 2;
        let first = SafepointEntry {
            pc_offset: 1,
            register_mask,
            register_ids,
            ..entry()
        };
        let second = SafepointEntry {
            pc_offset: 7,
            ..entry()
        };
        let bytes = must_encode(&[group(vec![first, second])]);
        let end = 12 + first_entry_length + prefix;
        assert_eq!(
            decode_section(&bytes[..end], 500, &layout),
            Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                500 + 12 + first_entry_length + field_offset
            )),
            "second entry prefix length {prefix}"
        );
    }
}

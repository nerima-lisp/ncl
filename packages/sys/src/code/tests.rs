use super::*;
use crate::{LowTag, Word};

#[test]
fn code_lifecycle_and_write_bounds() {
    let Ok(mut code) = alloc_code(4) else {
        return;
    };
    assert!(!code.is_published());
    assert_eq!(code.write_code(0, &[1, 2, 3, 4]), Ok(()));
    assert_eq!(code.write_code(3, &[5, 6]), Err(CodeError::OutOfBounds));
    assert!(publish_code(&mut code).is_ok());
    assert!(code.is_published());
    assert_eq!(code.entry(), code.address());
    assert_eq!(code.write_code(0, &[1]), Err(CodeError::AlreadyPublished));
}

#[test]
fn code_allocation_boundaries_and_metadata_accessors() {
    assert!(matches!(alloc_code(0), Err(CodeError::EmptyAllocation)));
    let Ok(mut code) = alloc_code(1) else { return };
    let mut registry = CodeRegistry::default();
    assert_eq!(
        registry.register(
            &code,
            CodeObjectMetadata {
                entry_offset: 0,
                size: 1,
                frame_words: 0,
                function_name: "unpublished".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: SafepointMap::default(),
                debug_table: Vec::new(),
            }
        ),
        Ok(())
    );
    assert!(registry.find(code.address()).is_none());
    assert_eq!(code.len(), 1);
    assert!(!code.is_empty());
    assert_eq!(code.as_slice(), &[0]);
    assert_eq!(
        code.write_code(usize::MAX, &[1]),
        Err(CodeError::OutOfBounds)
    );
    assert_eq!(code.write_code(0, &[7]), Ok(()));
    assert_eq!(code.as_slice(), &[7]);
    assert_eq!(code.entry(), 0);
    assert!(publish_code(&mut code).is_ok());
    assert_eq!(activate_code_registry(&code), Ok(()));
    assert!(registry.find(code.address()).is_some());
    assert_eq!(publish_code(&mut code), Err(CodeError::AlreadyPublished));
    free_code(code);
}

#[test]
fn frame_walk_and_scan_reject_invalid_ranges() {
    let words = [Word::NIL, Word::NIL, Word::NIL];
    assert!(walk_frame_headers(&words, 0, 1).is_empty());
    let map = Safepoint {
        pc_offset: 0,
        frame_words: 5,
        slot_words: 5,
        word_slot_count: 5,
        register_mask: 0,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: Vec::new(),
    };
    let mut frame = [Word::NIL; 2];
    assert_eq!(scan_frame(&mut frame, 0, &map, |word| word), None);
    assert_eq!(
        scan_frame_chain(&mut frame, 0, 0, &SafepointMap::default(), |word| word),
        None
    );
}

#[test]
fn frame_header_walk_and_register_scan_forward_roots() {
    let words = [
        Word::from_bits(8),
        Word::from_bits(0x1234),
        Word::fixnum(10),
        Word::from_bits(3),
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::from_bits(0x5678),
        Word::fixnum(20),
        Word::from_bits(4),
    ];
    let headers = walk_frame_headers(&words, 0, 2);
    assert_eq!(
        headers,
        [
            FrameHeader {
                previous: 8,
                return_pc: 0x1234,
                function: Word::fixnum(10),
                flags: 3,
            },
            FrameHeader {
                previous: 0,
                return_pc: 0x5678,
                function: Word::fixnum(20),
                flags: 4,
            },
        ]
    );

    let map = Safepoint {
        pc_offset: 7,
        frame_words: 4,
        slot_words: 4,
        word_slot_count: 3,
        register_mask: 1 << 3,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: vec![3],
    };
    let mut frame = [Word::NIL, Word::NIL, Word::fixnum(1), Word::NIL];
    let mut registers = [Word::NIL, Word::NIL, Word::NIL, Word::fixnum(9)];
    let mut saw_non_fixnum = false;
    assert_eq!(
        scan_frame_with_registers(&mut frame, 0, &map, &mut registers, |word| {
            word.as_fixnum().map_or_else(
                || {
                    saw_non_fixnum = true;
                    word
                },
                |value| Word::fixnum(value + 1),
            )
        }),
        Some(2)
    );
    assert!(!saw_non_fixnum, "only live fixnums are forwarded");
    assert_eq!(frame[2].as_fixnum(), Some(2));
    assert_eq!(registers[3].as_fixnum(), Some(10));

    let mut short_registers = [Word::NIL; 3];
    let mut frame = [Word::NIL, Word::NIL, Word::fixnum(4), Word::NIL];
    let mut saw_non_fixnum_frame_word = false;
    assert_eq!(
        scan_frame_with_registers(&mut frame, 0, &map, &mut short_registers, |word| {
            word.as_fixnum().map_or_else(
                || {
                    saw_non_fixnum_frame_word = true;
                    word
                },
                |value| Word::fixnum(value + 1),
            )
        }),
        Some(1)
    );
    assert!(
        !saw_non_fixnum_frame_word,
        "only the live frame slot is forwarded"
    );

    let mut invalid = map;
    invalid.register_ids = vec![16];
    assert_eq!(
        scan_frame_with_registers(&mut frame, 0, &invalid, &mut registers, |word| word),
        None
    );
}

#[test]
fn registry_register_find_unregister() {
    let Ok(mut code) = alloc_code(16) else {
        return;
    };
    assert!(publish_code(&mut code).is_ok());
    let metadata = CodeObjectMetadata {
        entry_offset: 0,
        size: code.len(),
        frame_words: 0,
        function_name: "test".to_string(),
        source_locations: Vec::new(),
        constant_slots: vec![Word::NIL],
        safepoint_map: SafepointMap::default(),
        debug_table: Vec::new(),
    };
    let mut registry = CodeRegistry::default();
    assert!(registry.register(&code, metadata).is_ok());
    assert_eq!(
        registry.find(code.address() + 3).map(|(_, offset)| offset),
        Some(3)
    );
    let Some((found, offset)) = registry.find(code.address() + 3) else {
        return;
    };
    assert_eq!(found.entry_offset, 0);
    assert_eq!(offset, 3);
    assert!(registry.find(code.address() + code.len()).is_none());
    assert!(registry.unregister(&code).is_some());
    assert!(registry.find(code.address()).is_none());
}

#[test]
fn registry_rejects_invalid_metadata_and_duplicate_ranges() {
    let code_result = alloc_code(8);
    assert!(code_result.is_ok(), "allocate code range: {code_result:?}");
    let Ok(mut code) = code_result else {
        return;
    };
    let mut registry = CodeRegistry::default();
    let metadata = |size, entry_offset, function_name| CodeObjectMetadata {
        entry_offset,
        size,
        frame_words: 0,
        function_name,
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: SafepointMap::default(),
        debug_table: Vec::new(),
    };

    for invalid in [
        metadata(0, 0, "empty".to_owned()),
        metadata(9, 0, "oversized".to_owned()),
        metadata(8, 8, "entry-at-end".to_owned()),
    ] {
        assert_eq!(
            registry.register(&code, invalid),
            Err(CodeError::OutOfBounds)
        );
    }

    assert_eq!(
        registry.register(&code, metadata(8, 0, "registered".to_owned())),
        Ok(())
    );
    assert_eq!(
        registry.register(&code, metadata(8, 0, "duplicate".to_owned())),
        Err(CodeError::AlreadyRegistered)
    );
    assert!(registry.find(code.address()).is_none());

    assert_eq!(seal_code(&mut code), Ok(()));
    assert_eq!(synchronize_code(&mut code), Ok(()));
    assert_eq!(activate_code(&mut code), Ok(()));
    assert_eq!(activate_code_registry(&code), Ok(()));
    assert_eq!(
        registry
            .find(code.address())
            .map(|(metadata, _)| metadata.function_name.as_str()),
        Some("registered")
    );
}

#[test]
fn prepared_registry_metadata_stays_hidden_and_is_removed_on_failure() {
    let Ok(mut code) = alloc_code(8) else { return };
    let mut registry = CodeRegistry::default();
    let metadata = CodeObjectMetadata {
        entry_offset: 0,
        size: code.len(),
        frame_words: 0,
        function_name: "prepared".to_string(),
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: SafepointMap::default(),
        debug_table: Vec::new(),
    };
    assert_eq!(registry.register(&code, metadata), Ok(()));
    assert!(registry.find(code.address()).is_none());
    assert_eq!(activate_code_registry(&code), Err(CodeError::NotPublished));
    assert_eq!(activate_code(&mut code), Err(CodeError::NotSealed));
    assert!(registry.find(code.address()).is_none());
    assert!(registry.unregister(&code).is_some());
    assert!(registry.find(code.address()).is_none());
}

#[test]
fn registry_activation_waits_for_sealing_and_cache_synchronization() {
    let Ok(mut code) = alloc_code(8) else { return };
    let mut registry = CodeRegistry::default();
    assert_eq!(
        registry.register(
            &code,
            CodeObjectMetadata {
                entry_offset: 0,
                size: code.len(),
                frame_words: 0,
                function_name: "activation".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: SafepointMap::default(),
                debug_table: Vec::new(),
            }
        ),
        Ok(())
    );
    assert_eq!(activate_code_registry(&code), Err(CodeError::NotPublished));
    assert_eq!(activate_code(&mut code), Err(CodeError::NotSealed));
    assert_eq!(seal_code(&mut code), Ok(()));
    assert_eq!(
        activate_code(&mut code),
        Err(CodeError::CacheNotSynchronized)
    );
    assert!(registry.find(code.address()).is_none());
    assert_eq!(synchronize_code(&mut code), Ok(()));
    assert_eq!(activate_code(&mut code), Ok(()));
    assert!(registry.find(code.address()).is_none());
    assert_eq!(activate_code_registry(&code), Ok(()));
    assert_eq!(
        registry.find(code.address()).map(|(_, offset)| offset),
        Some(0)
    );
    assert_eq!(
        activate_code_registry(&code),
        Err(CodeError::AlreadyActivated)
    );
}

#[test]
fn map_decode_lookup_and_scan() {
    let mut bytes = vec![0; 16];
    bytes[0..4].copy_from_slice(&7u32.to_le_bytes());
    bytes[4..6].copy_from_slice(&8u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&5u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&5u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&8u16.to_le_bytes());
    bytes.push(0b0001_0100);
    bytes.extend_from_slice(&3u16.to_le_bytes());
    let Ok(map) = SafepointMap::decode(&bytes, 1) else {
        return;
    };
    let Some(entry) = map.find_map(8) else {
        return;
    };
    assert_eq!(entry.register_ids, [3]);
    let mut frame = [Word::NIL; 8];
    frame[2] = Word::fixnum(1);
    frame[4] = Word::fixnum(2);
    assert_eq!(
        scan_frame(&mut frame, 0, entry, |word| Word::fixnum(
            word.as_fixnum().unwrap_or(0) + 1
        )),
        Some(2)
    );
    assert_eq!(frame[2].as_fixnum(), Some(2));
    assert_eq!(frame[4].as_fixnum(), Some(3));
}

#[test]
fn safepoint_decode_rejects_register_ids_outside_register_mask() {
    let mut bytes = vec![0; 16];
    bytes[4..6].copy_from_slice(&4u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&3u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
    bytes.push(0b0000_0100);
    bytes.extend_from_slice(&3u16.to_le_bytes());
    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::RegisterMaskMismatch)
    );
}

#[test]
fn map_decode_rejects_truncated_and_invalid_contracts() {
    assert_eq!(
        SafepointMap::decode(&[], 1),
        Err(SafepointMapError::TruncatedHeader)
    );

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::InvalidSlotCounts)
    );

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::TruncatedSlotBitmap)
    );

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes.push(0);
    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::InvalidHeaderBitmap)
    );

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes.push(0b0000_0100);
    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::TruncatedRegisterIds)
    );
}

#[test]
fn safepoint_map_accessors_report_entries_and_liveness() {
    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes.push(0b0000_0100);
    let map = SafepointMap::decode(&bytes, 1).unwrap_or_default();
    assert_eq!(map.entries().len(), 1);
    assert!(SafepointMap::is_slot_live(&map.entries()[0], 2));
    assert!(!SafepointMap::is_slot_live(&map.entries()[0], 3));
}

#[test]
fn frame_chain_forwards_function_and_live_slots() {
    let mut bytes = vec![0; 16];
    bytes[0..4].copy_from_slice(&0u32.to_le_bytes());
    bytes[4..6].copy_from_slice(&5u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&5u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&5u16.to_le_bytes());
    bytes.push(0b0001_0100);
    let Ok(map) = SafepointMap::decode(&bytes, 1) else {
        return;
    };
    let mut frames = [
        Word::pointer(8, LowTag::OtherPointer),
        Word::from_bits(100),
        Word::fixnum(1),
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::pointer(0, LowTag::OtherPointer),
        Word::from_bits(104),
        Word::fixnum(2),
        Word::NIL,
        Word::NIL,
    ];
    assert_eq!(
        scan_frame_chain(&mut frames, 0, 96, &map, |word| Word::fixnum(
            word.as_fixnum().unwrap_or(0) + 1
        )),
        Some(4)
    );
    assert_eq!(frames[2].as_fixnum(), Some(2));
    assert_eq!(frames[10].as_fixnum(), Some(3));
}

#[test]
fn frame_chain_keeps_return_pc_low_bits_for_safepoint_lookup() {
    let mut bytes = vec![0; 16];
    bytes[0..4].copy_from_slice(&4u32.to_le_bytes());
    bytes[4..6].copy_from_slice(&4u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&3u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes.push(0b0000_0100);
    let Ok(map) = SafepointMap::decode(&bytes, 1) else {
        return;
    };
    let mut frame = [
        Word::from_bits(0),
        Word::from_bits(0x1004),
        Word::fixnum(1),
        Word::NIL,
    ];
    assert_eq!(
        scan_frame_chain(&mut frame, 0, 0x1000, &map, |word| {
            Word::fixnum(word.as_fixnum().unwrap_or(0) + 1)
        }),
        Some(1)
    );
    assert_eq!(frame[2], Word::fixnum(2));
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
mod invocation;

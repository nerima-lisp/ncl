use super::*;
use crate::LowTag;

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
        Err(CodeError::NotPublished)
    );
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
fn map_decode_lookup_and_scan() {
    let mut bytes = vec![0; 16];
    bytes[0..4].copy_from_slice(&7u32.to_le_bytes());
    bytes[4..6].copy_from_slice(&8u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&5u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&5u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
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
fn map_decode_rejects_truncated_and_invalid_contracts() {
    assert_eq!(SafepointMap::decode(&[], 1), Err("truncated safepoint header"));

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(SafepointMap::decode(&bytes, 1), Err("invalid slot counts"));

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(SafepointMap::decode(&bytes, 1), Err("truncated slot bitmap"));

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes.push(0);
    assert_eq!(SafepointMap::decode(&bytes, 1), Err("invalid header bitmap"));

    let mut bytes = vec![0; 16];
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes.push(0b0000_0100);
    assert_eq!(SafepointMap::decode(&bytes, 1), Err("truncated register ids"));
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
#[test]
fn published_machine_code_returns_42() {
    #[cfg(target_arch = "aarch64")]
    let bytes = [0x40, 0x05, 0x80, 0xd2, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "x86_64")]
    let bytes = [0xb8, 0x2a, 0, 0, 0, 0xc3];
    let Ok(mut code) = alloc_code(bytes.len()) else {
        return;
    };
    assert!(code.write_code(0, &bytes).is_ok());
    assert!(publish_code(&mut code).is_ok());
    let mut thread = crate::Thread::new();
    let (value, _count) = crate::invoke_entry(&code, 0, &raw mut thread, 0, [0; 4], 0);
    assert_eq!(value, 42);
}

/// Generated code reads the pinned context register and the callee function
/// object that a native call supplies, so the invoke entry must deliver both.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn invoke_entry_delivers_context_and_function_object() {
    #[cfg(target_arch = "aarch64")]
    let context_bytes = [0xe0, 0x03, 0x15, 0xaa, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "aarch64")]
    let function_bytes = [0xe0, 0x03, 0x10, 0xaa, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "x86_64")]
    let context_bytes = [0x4c, 0x89, 0xf8, 0xc3];
    #[cfg(target_arch = "x86_64")]
    let function_bytes = [0x4c, 0x89, 0xd0, 0xc3];
    let mut thread = crate::Thread::new();
    let context = &raw mut thread;
    let function_object = 0x1234_5678_9abc_def0_u64;

    let Ok(mut context_code) = alloc_code(context_bytes.len()) else {
        return;
    };
    assert!(context_code.write_code(0, &context_bytes).is_ok());
    assert!(publish_code(&mut context_code).is_ok());
    let (returned_context, _) = crate::invoke_entry(&context_code, 0, context, 0, [0; 4], 0);
    assert_eq!(returned_context, context as usize as u64);

    let Ok(mut function_code) = alloc_code(function_bytes.len()) else {
        return;
    };
    assert!(function_code.write_code(0, &function_bytes).is_ok());
    assert!(publish_code(&mut function_code).is_ok());
    let (returned_function, _) = crate::invoke_entry_with_function(
        &function_code,
        0,
        context,
        function_object,
        0,
        [0; 4],
        0,
    );
    assert_eq!(returned_function, function_object);
}

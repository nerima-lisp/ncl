use super::*;

#[test]
fn scan_frame_rejects_a_full_frame_without_a_live_function_slot() {
    let unmapped_function_slot = Safepoint {
        pc_offset: 0,
        frame_words: 5,
        slot_words: 5,
        word_slot_count: 5,
        register_mask: 0,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0011],
        register_ids: Vec::new(),
    };
    let mut full_frame = [Word::NIL; 5];
    let mut forwarded = false;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame(&mut full_frame, 0, &unmapped_function_slot, |word| {
            forwarded = true;
            word
        }),
        None
    );
    // check-added-lines: allow(panic) test-only assertions
    assert!(!forwarded);
}

#[test]
fn frame_walkers_reject_checked_add_overflow() {
    let words = [Word::NIL; 4];
    // check-added-lines: allow(panic) test-only assertions
    assert!(walk_frame_headers(&words, usize::MAX, 1).is_empty());

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
    let mut frame: [Word; 0] = [];
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(scan_frame(&mut frame, usize::MAX, &map, |word| word), None);
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain(
            &mut frame,
            usize::MAX,
            0,
            &SafepointMap::default(),
            |word| word,
        ),
        None
    );

    let registry = CodeRegistry::default();
    let mut registers: [Word; 0] = [];
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain_with_registry(&mut frame, usize::MAX, &registry, &mut registers, |word| {
            word
        },),
        None
    );
}

#[test]
fn frame_header_walk_stops_on_a_self_link() {
    let mut words = [Word::NIL; 12];
    words[8] = Word::from_bits(8);
    words[9] = Word::from_bits(0x1234);
    words[10] = Word::fixnum(10);
    words[11] = Word::from_bits(3);

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        walk_frame_headers(&words, 8, 2),
        [FrameHeader {
            previous: 8,
            return_pc: 0x1234,
            function: Word::fixnum(10),
            flags: 3,
        }]
    );
}

#[test]
fn scan_frame_with_registers_rejects_an_in_range_register_outside_the_mask() {
    let map = Safepoint {
        pc_offset: 0,
        frame_words: 4,
        slot_words: 4,
        word_slot_count: 3,
        register_mask: 1,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: vec![4],
    };
    let mut frame = [Word::NIL; 4];
    let mut registers = [Word::NIL; 5];
    let mut forwarded = false;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_with_registers(&mut frame, 0, &map, &mut registers, |word| {
            forwarded = true;
            word
        }),
        None
    );
    // check-added-lines: allow(panic) test-only assertions
    assert!(!forwarded);
}

#[test]
fn slot_liveness_rejects_a_missing_bitmap_byte() {
    let map = Safepoint {
        pc_offset: 0,
        frame_words: 9,
        slot_words: 9,
        word_slot_count: 9,
        register_mask: 0,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: Vec::new(),
    };

    // check-added-lines: allow(panic) test-only assertions
    assert!(!SafepointMap::is_slot_live(&map, 8));
}

#[test]
fn frame_chain_rejects_return_pc_before_code_base() {
    let mut frame = [Word::from_bits(0), Word::from_bits(0), Word::NIL, Word::NIL];

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain(&mut frame, 0, 1, &SafepointMap::default(), |word| word),
        None
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn frame_chain_rejects_return_pc_offset_outside_safepoint_format() {
    let mut frame = [
        Word::from_bits(0),
        Word::from_bits(u64::from(u32::MAX) + 1),
        Word::NIL,
        Word::NIL,
    ];

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain(&mut frame, 0, 0, &SafepointMap::default(), |word| word),
        None
    );
}

#[test]
fn frame_chain_with_registry_rejects_an_unknown_return_pc() {
    let mut frame = [
        Word::from_bits(0),
        Word::from_bits(0x1000),
        Word::NIL,
        Word::NIL,
    ];
    let registry = CodeRegistry::default();
    let mut registers: [Word; 0] = [];

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain_with_registry(&mut frame, 0, &registry, &mut registers, |word| word,),
        None
    );
}

#[test]
fn frame_chain_with_registry_rejects_a_registered_pc_without_a_map() -> Result<(), CodeError> {
    let mut code = alloc_code(8)?;
    publish_code(&mut code)?;
    let mut registry = CodeRegistry::default();
    registry.register(
        &code,
        CodeObjectMetadata {
            entry_offset: 0,
            size: code.len(),
            frame_words: 4,
            function_name: "empty-safepoint-map".to_owned(),
            source_locations: Vec::new(),
            constant_slots: Vec::new(),
            safepoint_map: SafepointMap::default(),
            debug_table: Vec::new(),
        },
    )?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        registry.find(code.address()).map(|(_, offset)| offset),
        Some(0)
    );

    let mut frame = [
        Word::from_bits(0),
        Word::from_bits(code.address() as u64),
        Word::fixnum(7),
        Word::NIL,
    ];
    let original_frame = frame;
    let mut registers: [Word; 0] = [];
    let mut forwarded = false;

    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        scan_frame_chain_with_registry(&mut frame, 0, &registry, &mut registers, |word| {
            forwarded = true;
            word
        }),
        None
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(frame, original_frame);
    // check-added-lines: allow(panic) test-only assertions
    assert!(!forwarded);
    Ok(())
}

use super::*;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test keeps three frame variants side by side"
)]
fn collection_scans_frames_across_registered_code_objects() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let mut codes = Vec::new();
    for register_id in 0..3_u16 {
        let Ok(mut code) = crate::alloc_code(16) else {
            return;
        };
        assert!(crate::publish_code(&mut code).is_ok());
        let mut bytes = vec![0; 16];
        bytes[4..6].copy_from_slice(&5_u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&5_u16.to_le_bytes());
        bytes[8..10].copy_from_slice(&5_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(1_u16 << register_id).to_le_bytes());
        bytes.push(0b0001_0100);
        bytes.extend_from_slice(&register_id.to_le_bytes());
        let Ok(map) = crate::SafepointMap::decode(&bytes, 1) else {
            return;
        };
        assert!(
            h.register_code(
                &code,
                crate::CodeObjectMetadata {
                    entry_offset: 0,
                    size: code.len(),
                    frame_words: 5,
                    function_name: format!("frame-{register_id}"),
                    source_locations: Vec::new(),
                    constant_slots: Vec::new(),
                    safepoint_map: map,
                    debug_table: Vec::new(),
                }
            )
            .is_ok()
        );
        codes.push(code);
    }
    let mut functions = Vec::new();
    let mut live_locals = Vec::new();
    for index in 0..3 {
        functions.push(
            h.alloc_cons(&mut t, Word::fixnum(index), Word::NIL)
                .unwrap_or(Word::NIL),
        );
        live_locals.push(
            h.alloc_cons(&mut t, Word::fixnum(index + 10), Word::NIL)
                .unwrap_or(Word::NIL),
        );
    }
    let old_functions = functions.clone();
    let old_locals = live_locals.clone();
    let registers = vec![functions[0], functions[1], functions[2]];
    let old_registers = registers.clone();
    t.set_frame_snapshot(
        vec![
            Word::pointer(8, crate::LowTag::OtherPointer),
            Word::from_bits(codes[0].address() as u64),
            functions[0],
            Word::fixnum(100),
            live_locals[0],
            Word::fixnum(900),
            Word::fixnum(901),
            Word::fixnum(902),
            Word::pointer(16, crate::LowTag::OtherPointer),
            Word::from_bits(codes[1].address() as u64),
            functions[1],
            Word::fixnum(200),
            live_locals[1],
            Word::fixnum(903),
            Word::fixnum(904),
            Word::fixnum(905),
            Word::pointer(0, crate::LowTag::OtherPointer),
            Word::from_bits(codes[2].address() as u64),
            functions[2],
            Word::fixnum(300),
            live_locals[2],
            Word::fixnum(906),
            Word::fixnum(907),
            Word::fixnum(908),
        ],
        registers,
    );
    h.collect(true);
    for (index, old) in [
        (2, old_functions[0]),
        (10, old_functions[1]),
        (18, old_functions[2]),
    ] {
        assert_ne!(t.frame_chain[index], old);
    }
    for (index, old) in [(4, old_locals[0]), (12, old_locals[1]), (20, old_locals[2])] {
        assert_ne!(t.frame_chain[index], old);
    }
    assert_eq!(t.frame_chain[3], Word::fixnum(100));
    assert_eq!(t.frame_chain[11], Word::fixnum(200));
    assert_eq!(t.frame_chain[19], Word::fixnum(300));
    for (updated, old) in t.frame_registers.iter().zip(old_registers) {
        assert_ne!(*updated, old);
    }
    let state = h.lock_state();
    for value in t
        .frame_chain
        .iter()
        .copied()
        .chain(t.frame_registers.iter().copied())
    {
        if value.is_cons() {
            assert!(Heap::find(&state, value).is_some());
        }
    }
}

#[test]
fn release_code_waits_for_registered_frames_to_quiesce() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let Ok(mut code) = crate::alloc_code(16) else {
        return;
    };
    assert!(crate::publish_code(&mut code).is_ok());
    assert!(
        h.register_code(
            &code,
            crate::CodeObjectMetadata {
                entry_offset: 0,
                size: code.len(),
                frame_words: 5,
                function_name: "release-test".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: crate::SafepointMap::default(),
                debug_table: Vec::new(),
            }
        )
        .is_ok()
    );
    t.set_frame_snapshot(
        vec![
            Word::from_bits(0),
            Word::from_bits(code.address() as u64),
            Word::NIL,
            Word::NIL,
            Word::NIL,
        ],
        Vec::new(),
    );
    let mut owned = Some(code);
    assert_eq!(
        h.release_code(&mut t, &mut owned),
        Err(crate::CodeError::CodeInUse)
    );
    assert!(owned.is_some());
    t.set_frame_snapshot(Vec::new(), Vec::new());
    assert!(h.release_code(&mut t, &mut owned).is_ok());
    assert!(owned.is_none());
}

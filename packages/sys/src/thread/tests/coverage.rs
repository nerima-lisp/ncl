use super::*;
use crate::{CodeObjectMetadata, CodeRegistry, SafepointMap, alloc_code, publish_code};

fn map_for_frame() -> SafepointMap {
    let mut bytes = vec![0; 16];
    bytes[4..6].copy_from_slice(&5_u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&5_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&5_u16.to_le_bytes());
    bytes.push(0b0001_0100);
    SafepointMap::decode(&bytes, 1).unwrap_or_default()
}

#[test]
fn native_poll_and_conservative_snapshot_cover_unregistered_paths() {
    let mut thread = Thread::new();
    thread.request_safepoint();
    thread.poll_safepoint();
    assert_eq!(thread.safepoint_state(), SafepointState::Running);
    thread.enter_native();
    thread.request_safepoint();
    thread.poll_safepoint();
    assert_eq!(thread.native_state(), NativeState::Native);
    thread.leave_native();
    thread.publish_conservative_root(Word::TRUE);
    let values = thread.conservative_snapshot();
    assert!(values.contains(&Word::TRUE));
    assert!(!thread.has_native_frame_snapshot());
    thread.heap_collect(false);
    assert!(!thread.frame_snapshot_failed());
}

#[test]
fn native_frame_capture_reads_registered_layout_and_writes_back() {
    let heap = crate::Heap::new(crate::HeapConfig::default());
    let mut thread = Thread::new();
    assert!(crate::register_thread(&heap, &mut thread).is_ok());
    let Ok(mut code) = alloc_code(16) else { return };
    assert!(publish_code(&mut code).is_ok());
    assert!(
        heap.register_code(
            &code,
            CodeObjectMetadata {
                entry_offset: 0,
                size: code.len(),
                frame_words: 5,
                function_name: "capture-test".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: map_for_frame(),
                debug_table: Vec::new(),
            }
        )
        .is_ok()
    );
    let mut storage = vec![Word::fixnum(9); 8];
    // SAFETY: the pointer stays within the live vector and the frame layout reads its initialized words.
    let frame_pointer = unsafe { storage.as_mut_ptr().add(3) } as usize;
    thread.capture_native_frame(frame_pointer, code.address());
    assert!(!thread.frame_snapshot_failed());
    assert!(thread.has_native_frame_snapshot());
    assert_eq!(
        thread.frame_word(1),
        Some(Word::from_bits(code.address() as u64))
    );
    thread.write_back_frame_snapshot();
    assert!(!thread.has_native_frame_snapshot());
    assert_eq!(thread.last_written_frame_word(0), Some(Word::fixnum(9)));
    crate::unregister_thread(&thread);
}

#[test]
fn native_frame_registry_scan_and_failed_capture_are_observable() {
    let Ok(mut code) = alloc_code(16) else { return };
    assert!(publish_code(&mut code).is_ok());
    let metadata = CodeObjectMetadata {
        entry_offset: 0,
        size: code.len(),
        frame_words: 5,
        function_name: "thread-test".to_string(),
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: map_for_frame(),
        debug_table: Vec::new(),
    };
    let mut registry = CodeRegistry::default();
    assert!(registry.register(&code, metadata).is_ok());
    let mut thread = Thread::new();
    thread.set_frame_snapshot(
        vec![
            Word::from_bits(0),
            Word::from_bits(code.address() as u64),
            Word::fixnum(1),
            Word::NIL,
            Word::fixnum(2),
        ],
        vec![Word::fixnum(3)],
    );
    assert_eq!(
        thread.scan_native_frame(&registry, |word| {
            Word::fixnum(word.as_fixnum().unwrap_or(0) + 1)
        }),
        Some(2)
    );
    assert_eq!(thread.frame_word(2), Some(Word::fixnum(2)));
    assert_eq!(thread.frame_word(99), None);
    thread.set_frame_snapshot(Vec::new(), Vec::new());
    assert!(!thread.has_native_frame_snapshot());
}

#[test]
fn native_frame_capture_marks_unknown_pc_as_failed() {
    let heap = crate::Heap::new(crate::HeapConfig::default());
    let mut thread = Thread::new();
    assert!(crate::register_thread(&heap, &mut thread).is_ok());
    thread.capture_native_frame(0, usize::MAX);
    assert!(thread.frame_snapshot_failed());
    assert!(!thread.has_native_frame_snapshot());
    crate::unregister_thread(&thread);
}

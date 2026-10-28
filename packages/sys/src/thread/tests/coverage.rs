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
    let mut root = Word::fixnum(7);
    let root_pointer = std::ptr::from_mut(&mut root);
    let root_token = thread.push_root(&mut root);
    thread.mv[0] = Word::TRUE;
    thread.mv_count = 1;
    let values = thread.conservative_snapshot();
    assert!(values.contains(&Word::TRUE));
    thread.publish_gc_state(0);
    let gc_state = thread.gc_state_handle();
    let published = gc_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(published.conservative_roots.contains(&Word::TRUE));
    assert_eq!(published.roots.len(), 1);
    assert_eq!(published.roots[0].as_ref().as_ptr(), root_pointer);
    assert_eq!(published.multiple_values[0], Word::TRUE);
    assert_eq!(published.mv_count, 1);
    drop(published);
    assert!(thread.pop_root(root_token));
    assert!(!thread.has_native_frame_snapshot());
    thread.heap_collect(false);
    assert!(!thread.frame_snapshot_failed());
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[test]
fn conservative_snapshot_uses_only_a_valid_stack_start_for_lisp_threads() {
    let first_word = Word::fixnum(17);
    let second_word = Word::fixnum(29);
    let stack_words = [first_word, second_word];
    let bounds_start = std::ptr::from_ref(&stack_words[0]).addr();
    let scan_start = std::ptr::from_ref(&stack_words[1]).addr();
    let bounds_end = bounds_start + std::mem::size_of_val(&stack_words);
    let mut thread = Thread::new();
    thread.stack_bounds = Some((bounds_start, bounds_end));
    thread.stack_scan_start = Some(scan_start);

    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(values.contains(&second_word));

    thread.stack_scan_start = Some(bounds_end);
    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(!values.contains(&second_word));

    thread.stack_scan_start = None;
    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(!values.contains(&second_word));
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[test]
fn conservative_snapshot_uses_valid_stack_start_for_native_threads() {
    let first_word = Word::fixnum(17);
    let second_word = Word::fixnum(29);
    let stack_words = [first_word, second_word];
    let bounds_start = std::ptr::from_ref(&stack_words[0]).addr();
    let scan_start = std::ptr::from_ref(&stack_words[1]).addr();
    let bounds_end = bounds_start + std::mem::size_of_val(&stack_words);
    assert_eq!(bounds_start % std::mem::align_of::<u64>(), 0);
    assert_eq!(scan_start % std::mem::align_of::<u64>(), 0);

    let mut thread = Thread::new();
    thread.stack_bounds = Some((bounds_start, bounds_end));
    thread.stack_scan_start = Some(scan_start);
    thread.native = NativeState::Native;

    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(values.contains(&second_word));

    thread.stack_scan_start = None;
    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(!values.contains(&second_word));

    thread.stack_scan_start = Some(bounds_end);
    let values = thread.conservative_snapshot();
    assert!(!values.contains(&first_word));
    assert!(!values.contains(&second_word));
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[test]
fn conservative_snapshot_uses_full_bounds_when_lisp_stack_start_may_be_stale() {
    let first_word = Word::fixnum(17);
    let second_word = Word::fixnum(29);
    let stack_words = [first_word, second_word];
    let bounds_start = std::ptr::from_ref(&stack_words[0]).addr();
    let scan_start = std::ptr::from_ref(&stack_words[1]).addr();
    let bounds_end = bounds_start + std::mem::size_of_val(&stack_words);
    let mut thread = Thread::new();
    thread.stack_bounds = Some((bounds_start, bounds_end));
    thread.stack_scan_start = Some(scan_start);

    let values = thread.conservative_snapshot();
    assert!(values.contains(&first_word));
    assert!(values.contains(&second_word));
}

#[test]
fn publish_gc_state_recovers_poisoned_mutex() {
    let thread = Thread::new();
    let gc_state = thread.gc_state_handle();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Ok(_guard) = gc_state.lock() {
            panic!("poison GC-state mutex");
        }
    }));
    assert!(panic.is_err());

    thread.publish_gc_state(42);

    let published = gc_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(published.published_epoch, 42);
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

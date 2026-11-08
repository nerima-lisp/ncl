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

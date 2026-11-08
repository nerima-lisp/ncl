use super::*;

fn pair() -> (FaslRelocation, FaslRelocation) {
    let adrp = FaslRelocation {
        code_object_id: 0,
        write_offset: 0,
        kind: FaslRelocationKind::Adrp21,
        target_kind: FaslRelocationTarget::CodeObject,
        target_index: 0,
        addend: 0,
        pair_id: 1,
    };
    let add = FaslRelocation {
        write_offset: 4,
        kind: FaslRelocationKind::Add12,
        ..adrp
    };
    (adrp, add)
}

#[test]
fn paired_instruction_check_rejects_missing_layout_and_truncated_words() {
    let (adrp, add) = pair();
    assert!(!pair_immediates_are_zero(&adrp, &add, &[]));

    let short_adrp = [FaslCodeLayout {
        id: 0,
        entry_offset: 0,
        frame_words: 0,
        constant_count: 0,
        code: &[0; 3],
    }];
    assert!(!pair_immediates_are_zero(&adrp, &add, &short_adrp));

    let short_add = [FaslCodeLayout {
        code: &[0; 4],
        ..short_adrp[0]
    }];
    assert!(!pair_immediates_are_zero(&adrp, &add, &short_add));
}

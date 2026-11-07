use super::*;
use crate::fasl_code::FaslCodeObject;
use crate::fasl_constants::{FaslConstant, FaslConstantGroup};
use crate::fasl_records::FaslCodeLayout;
use crate::fasl_stack_map::{SafepointEntry, SafepointGroup};
use crate::fasl_symbols::{FaslSymbol, SymbolBindingKind};
use crate::fasl_top_level::FaslTopLevelRecord;

const CODE: [u8; 16] = [0; 16];

fn layout() -> FaslCodeLayout<'static> {
    FaslCodeLayout {
        id: 0,
        entry_offset: 0,
        frame_words: 2,
        constant_count: 1,
        code: &CODE,
    }
}

fn assert_reason(result: Result<(), FaslError>, reason: FaslRejectReason) {
    assert!(matches!(result, Err(error) if error.reason == reason));
}

fn native_symbol(binding_kind: SymbolBindingKind) -> FaslSymbol {
    FaslSymbol {
        id: 0,
        binding_kind,
        package: "C".to_owned(),
        name: "puts".to_owned(),
    }
}

#[test]
fn references_validate_symbol_kind_and_all_constant_index_shapes() {
    let symbols = [native_symbol(SymbolBindingKind::Lisp)];
    let bad_constants = [
        FaslConstant::SymbolReference(1),
        FaslConstant::Cons { car: 1, cdr: 0 },
        FaslConstant::Ratio {
            numerator: 0,
            denominator: 1,
        },
        FaslConstant::Complex {
            real: 1,
            imaginary: 0,
        },
        FaslConstant::SimpleVector(vec![1]),
    ];
    for constant in bad_constants {
        let groups = [FaslConstantGroup {
            code_object_id: 0,
            constants: vec![constant],
        }];
        assert_reason(
            validate_references(&groups, &symbols),
            FaslRejectReason::InvalidReference,
        );
    }

    let groups = [FaslConstantGroup {
        code_object_id: 0,
        constants: vec![
            FaslConstant::SymbolReference(0),
            FaslConstant::Cons { car: 0, cdr: 1 },
            FaslConstant::Ratio {
                numerator: 0,
                denominator: 1,
            },
            FaslConstant::Complex {
                real: 1,
                imaginary: 2,
            },
            FaslConstant::SimpleVector(vec![0, 1, 2, 3, 4]),
            FaslConstant::Nil,
        ],
    }];
    assert_eq!(validate_references(&groups, &symbols), Ok(()));

    let wrong_binding = [native_symbol(SymbolBindingKind::NativeFunction)];
    let symbol_reference = [FaslConstantGroup {
        code_object_id: 0,
        constants: vec![FaslConstant::SymbolReference(0)],
    }];
    assert_reason(
        validate_references(&symbol_reference, &wrong_binding),
        FaslRejectReason::InvalidReference,
    );
}

#[test]
fn record_counts_reject_inconsistent_code_constant_stack_and_debug_records() {
    let code = [FaslCodeObject {
        id: 0,
        entry_offset: 0,
        frame_words: 2,
        constant_count: 0,
        stack_map_count: 0,
        debug_record_count: 0,
        code: &CODE,
    }];
    let constants = [FaslConstantGroup {
        code_object_id: 0,
        constants: Vec::new(),
    }];
    let stack_maps = [SafepointGroup {
        code_object_id: 0,
        entries: Vec::new(),
    }];
    assert_eq!(
        validate_record_counts(&code, &constants, &stack_maps, &[]),
        Ok(())
    );

    assert_reason(
        validate_record_counts(&code, &[], &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );
    assert_reason(
        validate_record_counts(&code, &constants, &[], &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let debug_record = DebugRecord {
        code_object_id: 1,
        pc_start: 0,
        pc_end: 1,
        file_id: 0,
        line: 1,
        column: 1,
        source_start: 0,
        source_end: 0,
        variables: Vec::new(),
    };
    assert_reason(
        validate_record_counts(&code, &constants, &stack_maps, &[debug_record]),
        FaslRejectReason::InvalidReference,
    );

    let mut wrong_code = code.clone();
    wrong_code[0].id = 1;
    assert_reason(
        validate_record_counts(&wrong_code, &constants, &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let mut wrong_constant_id = constants.clone();
    wrong_constant_id[0].code_object_id = 1;
    assert_reason(
        validate_record_counts(&code, &wrong_constant_id, &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let mut wrong_constant_count = code.clone();
    wrong_constant_count[0].constant_count = 1;
    assert_reason(
        validate_record_counts(&wrong_constant_count, &constants, &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let mut wrong_stack_id = stack_maps.clone();
    wrong_stack_id[0].code_object_id = 1;
    assert_reason(
        validate_record_counts(&code, &constants, &wrong_stack_id, &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let mut wrong_stack_count = code.clone();
    wrong_stack_count[0].stack_map_count = 1;
    assert_reason(
        validate_record_counts(&wrong_stack_count, &constants, &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );

    let mut wrong_debug_count = code.clone();
    wrong_debug_count[0].debug_record_count = 1;
    assert_reason(
        validate_record_counts(&wrong_debug_count, &constants, &stack_maps, &[]),
        FaslRejectReason::InvalidCodeObject,
    );
}

#[test]
fn stack_map_validation_checks_group_ids_pc_and_frame_size() {
    let layout = [layout()];
    assert_eq!(validate_stack_maps(&[], &[]), Ok(()));
    assert_reason(
        validate_stack_maps(&[], &layout),
        FaslRejectReason::InvalidSafepointMap,
    );

    let entry = SafepointEntry {
        pc_offset: 1,
        frame_words: 2,
        slot_words: 1,
        word_slot_count: 0,
        register_mask: 0,
        map_flags: 0,
        slot_bitmap: Vec::new(),
        register_ids: Vec::new(),
    };
    let valid_group = SafepointGroup {
        code_object_id: 0,
        entries: vec![entry.clone()],
    };
    assert_eq!(
        validate_stack_maps(std::slice::from_ref(&valid_group), &layout),
        Ok(())
    );

    let wrong_id = [SafepointGroup {
        code_object_id: 1,
        entries: Vec::new(),
    }];
    assert_reason(
        validate_stack_maps(&wrong_id, &layout),
        FaslRejectReason::InvalidSafepointMap,
    );

    let wrong_layout = [FaslCodeLayout { id: 1, ..layout[0] }];
    let group = [SafepointGroup {
        code_object_id: 0,
        entries: Vec::new(),
    }];
    assert_reason(
        validate_stack_maps(&group, &wrong_layout),
        FaslRejectReason::InvalidSafepointMap,
    );

    let out_of_range = [SafepointGroup {
        code_object_id: 0,
        entries: vec![SafepointEntry {
            pc_offset: 16,
            ..entry.clone()
        }],
    }];
    assert_reason(
        validate_stack_maps(&out_of_range, &layout),
        FaslRejectReason::InvalidSafepointMap,
    );

    let wrong_frame = [SafepointGroup {
        code_object_id: 0,
        entries: vec![SafepointEntry {
            frame_words: 3,
            ..entry
        }],
    }];
    assert_reason(
        validate_stack_maps(&wrong_frame, &layout),
        FaslRejectReason::InvalidSafepointMap,
    );
}

#[test]
fn top_level_validation_checks_ordinals_flags_code_and_entry() {
    let layouts = [layout()];
    assert_eq!(validate_top_level(&[], &[]), Ok(()));
    let valid = FaslTopLevelRecord {
        ordinal: 0,
        code_object_id: 0,
        entry_offset: 0,
        flags: 1,
    };
    assert_eq!(validate_top_level(&[valid], &layouts), Ok(()));

    let malformed = [
        FaslTopLevelRecord {
            ordinal: 1,
            ..valid
        },
        FaslTopLevelRecord {
            code_object_id: 1,
            ..valid
        },
        FaslTopLevelRecord { flags: 4, ..valid },
        FaslTopLevelRecord { flags: 2, ..valid },
        FaslTopLevelRecord {
            entry_offset: 1,
            ..valid
        },
    ];
    for record in malformed {
        assert_reason(
            validate_top_level(&[record], &layouts),
            FaslRejectReason::InvalidTopLevelRecord,
        );
    }
}

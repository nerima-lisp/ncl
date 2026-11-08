use super::*;
use crate::fasl::Architecture;
use crate::fasl_debug::{DebugFile, DebugLocationKind, DebugRecord, DebugVariable};
use crate::fasl_records::FaslCodeLayout;
use crate::fasl_relocation::{
    FaslRelocation, FaslRelocationKind as Kind, FaslRelocationTarget as Target,
};
use crate::fasl_symbols::{FaslSymbol, SymbolBindingKind};

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

fn relocation(
    kind: Kind,
    target_kind: Target,
    target_index: u32,
    write_offset: u32,
) -> FaslRelocation {
    FaslRelocation {
        code_object_id: 0,
        write_offset,
        kind,
        target_kind,
        target_index,
        addend: 0,
        pair_id: 0,
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
fn relocation_validation_rejects_sites_and_overlaps() {
    let source = [layout()];
    let abs64 = relocation(Kind::Abs64, Target::CodeObject, 0, 0);
    assert_eq!(
        validate_relocations(Architecture::X86_64, &[], &source, &[], None),
        Ok(())
    );
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[FaslRelocation {
                code_object_id: 1,
                ..abs64
            }],
            &source,
            &[],
            Some(128),
        ),
        FaslRejectReason::InvalidReference,
    );
    assert_eq!(
        validate_relocations(
            Architecture::X86_64,
            &[FaslRelocation {
                code_object_id: 1,
                ..abs64
            }],
            &source,
            &[],
            Some(128),
        )
        .map_err(|error| error.offset),
        Err(Some(132))
    );

    let out_of_bounds = relocation(Kind::Abs64, Target::CodeObject, 0, 12);
    assert_reason(
        validate_relocations(Architecture::X86_64, &[out_of_bounds], &source, &[], None),
        FaslRejectReason::InvalidRelocation,
    );

    let overlap = [
        relocation(Kind::Abs64, Target::CodeObject, 0, 0),
        relocation(Kind::Abs64, Target::CodeObject, 0, 4),
    ];
    assert_reason(
        validate_relocations(Architecture::X86_64, &overlap, &source, &[], None),
        FaslRejectReason::InvalidRelocation,
    );

    let too_many = vec![abs64; MAX_RECORDS + 1];
    assert_reason(
        validate_relocations(Architecture::X86_64, &too_many, &source, &[], None),
        FaslRejectReason::ResourceLimitExceeded,
    );
}

#[test]
fn relocation_validation_rejects_targets_architecture_and_pair_shapes() {
    let source = [layout()];
    let abs64 = relocation(Kind::Abs64, Target::CodeObject, 0, 0);
    let wrong_target_kind = relocation(Kind::PcRel32, Target::NativeFunction, 0, 0);
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[wrong_target_kind],
            &source,
            &[],
            None,
        ),
        FaslRejectReason::InvalidRelocation,
    );

    let wrong_architecture = relocation(Kind::Branch26, Target::CodeObject, 0, 0);
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[wrong_architecture],
            &source,
            &[],
            None,
        ),
        FaslRejectReason::InvalidRelocation,
    );

    let invalid_alignment = relocation(Kind::Adrp21, Target::CodeObject, 0, 2);
    assert_reason(
        validate_relocations(
            Architecture::Aarch64,
            &[invalid_alignment],
            &source,
            &[],
            None,
        ),
        FaslRejectReason::InvalidRelocation,
    );

    let invalid_pair_kind = FaslRelocation {
        pair_id: 1,
        ..abs64
    };
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[invalid_pair_kind],
            &source,
            &[],
            None,
        ),
        FaslRejectReason::InvalidRelocation,
    );

    let unpaired = FaslRelocation {
        pair_id: 1,
        kind: Kind::Adrp21,
        ..abs64
    };
    assert_reason(
        validate_relocations(Architecture::Aarch64, &[unpaired], &source, &[], None),
        FaslRejectReason::InvalidRelocation,
    );
}

#[test]
fn relocation_code_targets_require_valid_indices_and_entry_offsets() {
    let source = [layout()];
    let wrong_code_target = relocation(Kind::PcRel32, Target::CodeObject, 1, 0);
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[wrong_code_target],
            &source,
            &[],
            None,
        ),
        FaslRejectReason::InvalidReference,
    );

    for addend in [-1, 16, i64::MAX] {
        let bad_target = FaslRelocation {
            addend,
            ..relocation(Kind::PcRel32, Target::CodeObject, 0, 0)
        };
        assert_reason(
            validate_relocations(Architecture::X86_64, &[bad_target], &source, &[], None),
            FaslRejectReason::InvalidRelocation,
        );
    }

    let nonzero_entry = [FaslCodeLayout {
        entry_offset: 1,
        ..source[0]
    }];
    let overflowing_target = FaslRelocation {
        addend: i64::MAX,
        ..relocation(Kind::PcRel32, Target::CodeObject, 0, 0)
    };
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[overflowing_target],
            &nonzero_entry,
            &[],
            None,
        ),
        FaslRejectReason::InvalidRelocation,
    );
}

#[test]
fn relocation_native_targets_require_native_function_symbols() {
    let source = [layout()];
    let native = [native_symbol(SymbolBindingKind::NativeFunction)];
    assert_eq!(
        validate_relocations(
            Architecture::X86_64,
            &[relocation(
                Kind::ExternalSymbol,
                Target::NativeFunction,
                0,
                0
            )],
            &source,
            &native,
            None,
        ),
        Ok(())
    );
    let non_native = [native_symbol(SymbolBindingKind::Lisp)];
    assert_reason(
        validate_relocations(
            Architecture::X86_64,
            &[relocation(
                Kind::ExternalSymbol,
                Target::NativeFunction,
                0,
                0,
            )],
            &source,
            &non_native,
            None,
        ),
        FaslRejectReason::InvalidReference,
    );
}

#[test]
fn relocation_constant_targets_require_constant_index32_and_valid_indices() {
    let source = [layout()];
    for (kind, addend, target_index) in [
        (Kind::Abs64, 0, 0),
        (Kind::ConstantIndex32, 1, 0),
        (Kind::ConstantIndex32, 0, 1),
    ] {
        let bad_constant = FaslRelocation {
            addend,
            ..relocation(kind, Target::ConstantVector, target_index, 0)
        };
        assert_reason(
            validate_relocations(Architecture::X86_64, &[bad_constant], &source, &[], None),
            if kind == Kind::ConstantIndex32 && target_index == 1 {
                FaslRejectReason::InvalidReference
            } else {
                FaslRejectReason::InvalidRelocation
            },
        );
    }

    assert_eq!(
        validate_relocations(
            Architecture::X86_64,
            &[relocation(
                Kind::ConstantIndex32,
                Target::ConstantVector,
                0,
                0
            )],
            &source,
            &[],
            None,
        ),
        Ok(())
    );
}

#[test]
fn paired_aarch64_relocations_require_matching_zero_immediates() {
    let source = [layout()];
    let adrp = FaslRelocation {
        pair_id: 7,
        ..relocation(Kind::Adrp21, Target::CodeObject, 0, 0)
    };
    let add = FaslRelocation {
        pair_id: 7,
        ..relocation(Kind::Add12, Target::CodeObject, 0, 4)
    };
    let pair = [adrp, add];
    assert_eq!(
        validate_relocations(Architecture::Aarch64, &pair, &source, &[], None),
        Ok(())
    );

    let wrong_shape = [
        adrp,
        FaslRelocation {
            kind: Kind::Adrp21,
            ..add
        },
    ];
    assert_reason(
        validate_relocations(Architecture::Aarch64, &wrong_shape, &source, &[], None),
        FaslRejectReason::InvalidRelocation,
    );

    for (offset, immediate) in [(0_usize, 1_u32 << 29), (4, 1_u32 << 10)] {
        let mut code = CODE;
        code[offset..offset + 4].copy_from_slice(&immediate.to_le_bytes());
        let code_with_immediate = [FaslCodeLayout {
            code: &code,
            ..source[0]
        }];
        assert_reason(
            validate_relocations(
                Architecture::Aarch64,
                &pair,
                &code_with_immediate,
                &[],
                None,
            ),
            FaslRejectReason::InvalidRelocation,
        );
    }
}

#[test]
fn debug_validation_checks_file_and_record_ranges() {
    let layouts = [layout()];
    let file = DebugFile {
        file_id: 0,
        path: "src/main.lisp".to_owned(),
    };
    assert_eq!(
        fasl_debug::validate(std::slice::from_ref(&file), &[], &layouts),
        Ok(())
    );
    assert_reason(
        fasl_debug::validate(
            &[DebugFile {
                file_id: 0,
                path: "../main.lisp".to_owned(),
            }],
            &[],
            &layouts,
        ),
        FaslRejectReason::InvalidDebugRecord,
    );

    let valid_record = DebugRecord {
        code_object_id: 0,
        pc_start: 1,
        pc_end: 2,
        file_id: 0,
        line: 1,
        column: 1,
        source_start: 0,
        source_end: 1,
        variables: vec![DebugVariable {
            name: "x".to_owned(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        }],
    };
    assert_eq!(
        fasl_debug::validate(
            std::slice::from_ref(&file),
            std::slice::from_ref(&valid_record),
            &layouts,
        ),
        Ok(())
    );
    assert_reason(
        fasl_debug::validate(
            std::slice::from_ref(&file),
            &[DebugRecord {
                code_object_id: 1,
                ..valid_record.clone()
            }],
            &layouts,
        ),
        FaslRejectReason::InvalidReference,
    );
    let invalid_records = [
        DebugRecord {
            pc_start: 2,
            pc_end: 1,
            ..valid_record.clone()
        },
        DebugRecord {
            pc_end: 17,
            ..valid_record.clone()
        },
        DebugRecord {
            file_id: 1,
            ..valid_record.clone()
        },
        DebugRecord {
            line: 0,
            ..valid_record.clone()
        },
        DebugRecord {
            column: 0,
            ..valid_record.clone()
        },
        DebugRecord {
            source_start: 1,
            source_end: 0,
            ..valid_record
        },
    ];
    for record in invalid_records {
        assert_reason(
            fasl_debug::validate(std::slice::from_ref(&file), &[record], &layouts),
            FaslRejectReason::InvalidDebugRecord,
        );
    }
}

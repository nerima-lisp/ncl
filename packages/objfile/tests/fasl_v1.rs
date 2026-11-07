//! Integration coverage for native FASL v1 container files.

use ncl_objfile::{
    Architecture, DebugFile, DebugLocationKind, DebugRecord, DebugVariable, FaslCodeObject,
    FaslConstant, FaslConstantGroup, FaslError, FaslMetadata, FaslRejectReason, FaslRelocation,
    FaslRelocationKind as Kind, FaslRelocationTarget as Target, FaslSymbol, FaslTopLevelRecord,
    FaslV1, FaslV1Compatibility, FaslV1Reader, FaslV1Writer, FeatureRecord, PolicyRecord,
    SafepointEntry, SafepointGroup, SymbolBindingKind, compiler_identity, feature_fingerprint,
    policy_fingerprint,
};

const CODE: [u8; 64] = [0; 64];

fn metadata(target_triple: &str) -> FaslMetadata {
    FaslMetadata {
        semantic_version: "1.2.3".to_owned(),
        compiler_build_identity: [0x11; 32],
        codegen_abi_revision: 7,
        target_triple: target_triple.to_owned(),
        source_sha256: [0x22; 32],
        policies: vec![
            PolicyRecord { id: 1, value: 2 },
            PolicyRecord { id: 3, value: 1 },
        ],
        features: vec![
            FeatureRecord {
                name: "ncl/arm".to_owned(),
                version: "1".to_owned(),
            },
            FeatureRecord {
                name: "ncl/core".to_owned(),
                version: "1".to_owned(),
            },
        ],
    }
}

fn fixture(architecture: Architecture) -> FaslV1<'static> {
    let target_triple = match architecture {
        Architecture::X86_64 => "x86_64-unknown-linux-gnu",
        Architecture::Aarch64 => "aarch64-unknown-linux-gnu",
    };
    let relocations = match architecture {
        Architecture::X86_64 => vec![
            relocation(0, Kind::Abs64, Target::CodeObject, 0, 0),
            relocation(8, Kind::PcRel32, Target::CodeObject, 0, 0),
            relocation(12, Kind::Plt32, Target::NativeFunction, 1, 0),
            relocation(16, Kind::CodeEntry, Target::CodeObject, 0, 0),
            relocation(24, Kind::ExternalSymbol, Target::NativeFunction, 1, 0),
            relocation(32, Kind::ConstantIndex32, Target::ConstantVector, 1, 0),
        ],
        Architecture::Aarch64 => vec![
            relocation(0, Kind::Abs64, Target::CodeObject, 0, 0),
            relocation(8, Kind::Branch26, Target::CodeObject, 0, 0),
            relocation_pair(12, Kind::Adrp21, 4),
            relocation_pair(16, Kind::Add12, 4),
            relocation(20, Kind::CondBranch19, Target::CodeObject, 0, 0),
            relocation(24, Kind::CodeEntry, Target::CodeObject, 0, 0),
            relocation(32, Kind::ExternalSymbol, Target::NativeFunction, 1, 0),
            relocation(40, Kind::ConstantIndex32, Target::ConstantVector, 1, 0),
        ],
    };
    FaslV1 {
        architecture,
        runtime_abi_version: 9,
        metadata: metadata(target_triple),
        symbols: vec![
            FaslSymbol {
                id: 0,
                binding_kind: SymbolBindingKind::Lisp,
                package: "CORE".to_owned(),
                name: "X".to_owned(),
            },
            FaslSymbol {
                id: 1,
                binding_kind: SymbolBindingKind::NativeFunction,
                package: "C".to_owned(),
                name: "puts".to_owned(),
            },
        ],
        constants: vec![FaslConstantGroup {
            code_object_id: 0,
            constants: vec![FaslConstant::Nil, FaslConstant::SymbolReference(0)],
        }],
        code: vec![FaslCodeObject {
            id: 0,
            entry_offset: 0,
            frame_words: 3,
            constant_count: 2,
            stack_map_count: 1,
            debug_record_count: 1,
            code: &CODE,
        }],
        relocations,
        stack_maps: vec![SafepointGroup {
            code_object_id: 0,
            entries: vec![SafepointEntry {
                pc_offset: 2,
                frame_words: 3,
                slot_words: 3,
                word_slot_count: 1,
                register_mask: 0,
                map_flags: 0,
                slot_bitmap: vec![0b0000_0100],
                register_ids: Vec::new(),
            }],
        }],
        debug_files: vec![DebugFile {
            file_id: 0,
            path: "src/main.lisp".to_owned(),
        }],
        debug_records: vec![DebugRecord {
            code_object_id: 0,
            pc_start: 1,
            pc_end: 5,
            file_id: 0,
            line: 1,
            column: 1,
            source_start: 0,
            source_end: 4,
            variables: vec![DebugVariable {
                name: "x".to_owned(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 2,
            }],
        }],
        top_level: vec![FaslTopLevelRecord {
            ordinal: 0,
            code_object_id: 0,
            entry_offset: 0,
            flags: 3,
        }],
    }
}

fn relocation(
    write_offset: u32,
    kind: Kind,
    target_kind: Target,
    target_index: u32,
    pair_id: u32,
) -> FaslRelocation {
    FaslRelocation {
        code_object_id: 0,
        write_offset,
        kind,
        target_kind,
        target_index,
        addend: 0,
        pair_id,
    }
}

fn relocation_pair(write_offset: u32, kind: Kind, pair_id: u32) -> FaslRelocation {
    relocation(write_offset, kind, Target::CodeObject, 0, pair_id)
}

fn compatibility(fasl: &FaslV1<'_>) -> FaslV1Compatibility {
    FaslV1Compatibility {
        architecture: fasl.architecture,
        target_triple: fasl.metadata.target_triple.clone(),
        runtime_abi_version: fasl.runtime_abi_version,
        compiler_identity: compiler_identity(&fasl.metadata).expect("metadata identity is valid"),
        policy_fingerprint: policy_fingerprint(&fasl.metadata.policies)
            .expect("policies are valid"),
        feature_fingerprint: feature_fingerprint(&fasl.metadata.features)
            .expect("features are valid"),
    }
}

fn section_offset(bytes: &[u8], section_index: usize) -> usize {
    let offset = 136 + section_index * 24;
    usize::try_from(u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]))
    .expect("fixture offset fits usize")
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn decode_hex(source: &str) -> Vec<u8> {
    let source = source.trim();
    assert!(source.len().is_multiple_of(2));
    (0..source.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&source[index..index + 2], 16).expect("golden byte is hexadecimal")
        })
        .collect()
}

#[test]
fn complete_files_round_trip_deterministically_for_both_architectures() {
    for architecture in [Architecture::X86_64, Architecture::Aarch64] {
        let value = fixture(architecture);
        let first = FaslV1Writer::write(&value).expect("valid typed FASL encodes");
        let second = FaslV1Writer::write(&value).expect("same FASL encodes again");
        assert_eq!(first, second);
        assert_eq!(&first[..8], b"NCLFASL\0");
        assert_eq!(&first[8..14], &[3, 0, architecture as u8, 8, 1, 128]);
        let golden = match architecture {
            Architecture::X86_64 => {
                include_str!("golden/fasl-v1-x86_64.hex")
            }
            Architecture::Aarch64 => {
                include_str!("golden/fasl-v1-aarch64.hex")
            }
        };
        assert_eq!(first, decode_hex(golden));
        assert_eq!(
            FaslV1Reader::read(&first, &compatibility(&value)),
            Ok(value)
        );
    }
}

#[test]
fn reader_returns_each_stable_rejection_category() {
    let value = fixture(Architecture::Aarch64);
    let valid = FaslV1Writer::write(&value).expect("valid typed FASL encodes");
    let expected = compatibility(&value);

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 1;
    assert_reason(&bad_magic, &expected, FaslRejectReason::BadMagic);

    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_reason(
        &bad_version,
        &expected,
        FaslRejectReason::UnsupportedVersion,
    );

    let mut bad_target = valid.clone();
    bad_target[10] = 9;
    assert_reason(&bad_target, &expected, FaslRejectReason::TargetMismatch);
    let mut wrong_target_triple = expected.clone();
    wrong_target_triple.target_triple = "x86_64-unknown-linux-gnu".to_owned();
    assert_reason(
        &valid,
        &wrong_target_triple,
        FaslRejectReason::TargetMismatch,
    );

    let mut wrong_runtime = expected.clone();
    wrong_runtime.runtime_abi_version += 1;
    assert_reason(&valid, &wrong_runtime, FaslRejectReason::RuntimeAbiMismatch);

    let mut bad_compiler = valid.clone();
    bad_compiler[24] ^= 1;
    assert_reason(
        &bad_compiler,
        &expected,
        FaslRejectReason::CompilerIdentityMismatch,
    );

    let mut bad_policy = valid.clone();
    bad_policy[56] ^= 1;
    assert_reason(
        &bad_policy,
        &expected,
        FaslRejectReason::PolicyFingerprintMismatch,
    );

    let mut bad_features = valid.clone();
    bad_features[88] ^= 1;
    assert_reason(
        &bad_features,
        &expected,
        FaslRejectReason::FeatureFingerprintMismatch,
    );

    let mut bad_directory = valid.clone();
    bad_directory[128] = 0;
    assert_reason(
        &bad_directory,
        &expected,
        FaslRejectReason::MalformedDirectory,
    );

    let mut over_limit = valid.clone();
    write_u32(&mut over_limit, 144, 0);
    over_limit[148..152].copy_from_slice(&(257_u32 * 1024 * 1024).to_le_bytes());
    assert_reason(
        &over_limit,
        &expected,
        FaslRejectReason::ResourceLimitExceeded,
    );

    let mut bad_metadata = valid.clone();
    write_u32(&mut bad_metadata, section_offset(&valid, 0), 2);
    assert_reason(
        &bad_metadata,
        &expected,
        FaslRejectReason::MalformedMetadata,
    );

    let mut bad_reference = valid.clone();
    let constants = section_offset(&valid, 2);
    write_u32(&mut bad_reference, constants + 28, 99);
    assert_reason(
        &bad_reference,
        &expected,
        FaslRejectReason::InvalidReference,
    );

    let mut bad_code = valid.clone();
    let code = section_offset(&valid, 3);
    bad_code[code + 18] = 1;
    assert_reason(&bad_code, &expected, FaslRejectReason::InvalidCodeObject);

    let mut bad_relocation = valid.clone();
    let relocations = section_offset(&valid, 4);
    write_u32(&mut bad_relocation, relocations + 4 + 2 * 32 + 8, 99);
    assert_reason(
        &bad_relocation,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut bad_stack_map = valid.clone();
    let stack_maps = section_offset(&valid, 5);
    bad_stack_map[stack_maps + 16..stack_maps + 18].copy_from_slice(&0_u16.to_le_bytes());
    assert_reason(
        &bad_stack_map,
        &expected,
        FaslRejectReason::InvalidSafepointMap,
    );

    let mut bad_debug = valid.clone();
    let debug = section_offset(&valid, 6);
    write_u32(&mut bad_debug, debug + 48, 0);
    assert_reason(&bad_debug, &expected, FaslRejectReason::InvalidDebugRecord);

    let mut bad_top_level = valid.clone();
    let top_level = section_offset(&valid, 7);
    write_u32(&mut bad_top_level, top_level + 4, 1);
    assert_reason(
        &bad_top_level,
        &expected,
        FaslRejectReason::InvalidTopLevelRecord,
    );
}

#[test]
fn aarch64_relocation_pairs_reject_missing_halves_mismatches_and_immediates() {
    let value = fixture(Architecture::Aarch64);
    let expected = compatibility(&value);
    let valid = FaslV1Writer::write(&value).expect("valid AArch64 FASL encodes");
    let relocations = section_offset(&valid, 4);

    let mut missing_half = valid.clone();
    write_u32(&mut missing_half, relocations + 4 + 3 * 32 + 28, 0);
    assert_reason(
        &missing_half,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut mismatched_addend = valid.clone();
    mismatched_addend[relocations + 4 + 3 * 32 + 20] = 1;
    assert_reason(
        &mismatched_addend,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut reversed = valid.clone();
    write_u32(&mut reversed, relocations + 4 + 2 * 32 + 8, 5);
    write_u32(&mut reversed, relocations + 4 + 3 * 32 + 8, 4);
    assert_reason(&reversed, &expected, FaslRejectReason::InvalidRelocation);

    let mut mismatched_target = valid.clone();
    write_u32(&mut mismatched_target, relocations + 4 + 3 * 32 + 12, 2);
    write_u32(&mut mismatched_target, relocations + 4 + 3 * 32 + 16, 1);
    assert_reason(
        &mismatched_target,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut non_adjacent = valid.clone();
    write_u32(&mut non_adjacent, relocations + 4 + 2 * 32 + 4, 44);
    write_u32(&mut non_adjacent, relocations + 4 + 3 * 32 + 4, 52);
    assert_reason(
        &non_adjacent,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut reused_pair_id = valid.clone();
    write_u32(&mut reused_pair_id, relocations + 4 + 4 * 32 + 8, 4);
    write_u32(&mut reused_pair_id, relocations + 4 + 4 * 32 + 28, 4);
    assert_reason(
        &reused_pair_id,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut nonzero_immediate = valid;
    let code = section_offset(&nonzero_immediate, 3);
    nonzero_immediate[code + 4 + 28 + 15] = 0x20;
    assert_reason(
        &nonzero_immediate,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );
}

#[test]
fn bounded_single_byte_mutations_never_escape_the_typed_reader() {
    for architecture in [Architecture::X86_64, Architecture::Aarch64] {
        let value = fixture(architecture);
        let bytes = FaslV1Writer::write(&value).expect("valid typed FASL encodes");
        let expected = compatibility(&value);
        let mut mutations = 0_usize;
        for (index, original) in bytes.iter().copied().enumerate() {
            for replacement in [0, u8::MAX, original ^ 1] {
                if replacement == original {
                    continue;
                }
                let mut candidate = bytes.clone();
                candidate[index] = replacement;
                if let Ok(decoded) = FaslV1Reader::read(&candidate, &expected) {
                    assert_eq!(decoded.architecture, architecture);
                    assert_eq!(decoded.code.len(), value.code.len());
                }
                mutations += 1;
            }
        }
        assert!(mutations >= bytes.len());
    }
}

#[test]
fn x86_rejects_aarch64_only_relocation_tags_and_bad_target_kinds() {
    let value = fixture(Architecture::X86_64);
    let expected = compatibility(&value);
    let valid = FaslV1Writer::write(&value).expect("valid x86 FASL encodes");
    let relocations = section_offset(&valid, 4);

    let mut aarch64_kind = valid.clone();
    write_u32(&mut aarch64_kind, relocations + 4 + 32 + 8, 3);
    assert_reason(
        &aarch64_kind,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut bad_target_kind = valid.clone();
    write_u32(&mut bad_target_kind, relocations + 4 + 5 * 32 + 12, 1);
    assert_reason(
        &bad_target_kind,
        &expected,
        FaslRejectReason::InvalidRelocation,
    );

    let mut bad_constant_index = valid;
    write_u32(&mut bad_constant_index, relocations + 4 + 5 * 32 + 16, 2);
    assert_reason(
        &bad_constant_index,
        &expected,
        FaslRejectReason::InvalidReference,
    );
}

fn assert_reason(bytes: &[u8], expected: &FaslV1Compatibility, reason: FaslRejectReason) {
    assert!(matches!(
        FaslV1Reader::read(bytes, expected),
        Err(FaslError {
            reason: actual,
            ..
        }) if actual == reason
    ));
}

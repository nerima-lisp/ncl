use super::*;

fn fixture() -> FaslV1<'static> {
    FaslV1 {
        architecture: Architecture::X86_64,
        runtime_abi_version: 9,
        metadata: FaslMetadata {
            semantic_version: "1.0".to_owned(),
            compiler_build_identity: [1; 32],
            codegen_abi_revision: 1,
            target_triple: "x86_64-unknown-linux-gnu".to_owned(),
            source_sha256: [2; 32],
            policies: Vec::new(),
            features: Vec::new(),
        },
        symbols: Vec::new(),
        constants: Vec::new(),
        code: Vec::new(),
        relocations: Vec::new(),
        stack_maps: Vec::new(),
        debug_files: Vec::new(),
        debug_records: Vec::new(),
        top_level: Vec::new(),
    }
}

fn must_succeed<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("test fixture must be valid"))
}

fn compatibility(fasl: &FaslV1<'_>) -> FaslV1Compatibility {
    FaslV1Compatibility {
        architecture: fasl.architecture,
        target_triple: fasl.metadata.target_triple.clone(),
        runtime_abi_version: fasl.runtime_abi_version,
        compiler_identity: must_succeed(fasl_metadata::compiler_identity(&fasl.metadata)),
        policy_fingerprint: must_succeed(fasl_metadata::policy_fingerprint(
            &fasl.metadata.policies,
        )),
        feature_fingerprint: must_succeed(fasl_metadata::feature_fingerprint(
            &fasl.metadata.features,
        )),
    }
}

fn assert_reason(bytes: &[u8], expected: &FaslV1Compatibility, reason: FaslRejectReason) {
    assert!(matches!(
        FaslV1Reader::read(bytes, expected),
        Err(error) if error.reason == reason
    ));
}

fn section_start(bytes: &[u8], id: SectionId) -> usize {
    let index = match id {
        SectionId::Metadata => 0,
        SectionId::Symbols => 1,
        SectionId::Constants => 2,
        SectionId::Code => 3,
        SectionId::Relocations => 4,
        SectionId::StackMaps => 5,
        SectionId::DebugRecords => 6,
        SectionId::TopLevelRecords => 7,
    };
    let offset = 128 + index * 24 + 8;
    let file_offset = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    usize::try_from(file_offset).unwrap_or_else(|_| panic!("fixture offset fits usize"))
}

fn set_section_first_word(bytes: &mut [u8], id: SectionId, value: u32) {
    let start = section_start(bytes, id);
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn empty_native_container_round_trips_and_checks_expected_compatibility() {
    let value = fixture();
    let bytes = must_succeed(FaslV1Writer::write(&value));
    let expected = compatibility(&value);
    assert_eq!(FaslV1Reader::read(&bytes, &expected), Ok(value));

    let mut wrong_architecture = expected.clone();
    wrong_architecture.architecture = Architecture::Aarch64;
    assert_reason(
        &bytes,
        &wrong_architecture,
        FaslRejectReason::TargetMismatch,
    );

    let mut wrong_target = expected.clone();
    wrong_target.target_triple = "aarch64-unknown-linux-gnu".to_owned();
    assert_reason(&bytes, &wrong_target, FaslRejectReason::TargetMismatch);

    let mut wrong_abi = expected.clone();
    wrong_abi.runtime_abi_version += 1;
    assert_reason(&bytes, &wrong_abi, FaslRejectReason::RuntimeAbiMismatch);

    let mut wrong_compiler = expected.clone();
    wrong_compiler.compiler_identity[0] ^= 1;
    assert_reason(
        &bytes,
        &wrong_compiler,
        FaslRejectReason::CompilerIdentityMismatch,
    );

    let mut wrong_policy = expected.clone();
    wrong_policy.policy_fingerprint[0] ^= 1;
    assert_reason(
        &bytes,
        &wrong_policy,
        FaslRejectReason::PolicyFingerprintMismatch,
    );

    let mut wrong_feature = expected;
    wrong_feature.feature_fingerprint[0] ^= 1;
    assert_reason(
        &bytes,
        &wrong_feature,
        FaslRejectReason::FeatureFingerprintMismatch,
    );
}

#[test]
fn native_reader_rejects_header_fingerprints_and_short_directory() {
    let value = fixture();
    let expected = compatibility(&value);
    let bytes = must_succeed(FaslV1Writer::write(&value));

    for (offset, reason) in [
        (24, FaslRejectReason::CompilerIdentityMismatch),
        (56, FaslRejectReason::PolicyFingerprintMismatch),
        (88, FaslRejectReason::FeatureFingerprintMismatch),
    ] {
        let mut malformed = bytes.clone();
        malformed[offset] ^= 1;
        assert_reason(&malformed, &expected, reason);
    }

    assert_reason(
        &bytes[..319],
        &expected,
        FaslRejectReason::MalformedDirectory,
    );
}

#[test]
fn native_reader_propagates_rejections_from_every_section_decoder() {
    let value = fixture();
    let expected = compatibility(&value);
    let original = must_succeed(FaslV1Writer::write(&value));
    let cases = [
        (SectionId::Metadata, FaslRejectReason::MalformedMetadata),
        (SectionId::Symbols, FaslRejectReason::InvalidReference),
        (SectionId::Constants, FaslRejectReason::InvalidCodeObject),
        (SectionId::Code, FaslRejectReason::InvalidCodeObject),
        (SectionId::Relocations, FaslRejectReason::InvalidRelocation),
        (SectionId::StackMaps, FaslRejectReason::InvalidSafepointMap),
        (
            SectionId::DebugRecords,
            FaslRejectReason::InvalidDebugRecord,
        ),
        (
            SectionId::TopLevelRecords,
            FaslRejectReason::InvalidTopLevelRecord,
        ),
    ];

    for (id, reason) in cases {
        let mut malformed = original.clone();
        set_section_first_word(
            &mut malformed,
            id,
            if id == SectionId::Metadata { 0 } else { 1 },
        );
        assert_reason(&malformed, &expected, reason);
    }
}

#[test]
fn native_writer_reports_checked_output_ranges() {
    let mut bytes = [0_u8; 1];
    assert_eq!(
        write_at(&mut bytes, usize::MAX, &[1]),
        Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            usize::MAX
        ))
    );
    assert_eq!(
        write_at(&mut bytes, 1, &[1]),
        Err(FaslError::at(FaslRejectReason::MalformedDirectory, 1))
    );
}

#[test]
fn native_writer_rejects_invalid_section_records() {
    let mut bad_metadata = fixture();
    bad_metadata.metadata.semantic_version.clear();
    assert_eq!(
        FaslV1Writer::write(&bad_metadata),
        Err(FaslError::at(FaslRejectReason::MalformedMetadata, 0))
    );

    let mut bad_symbols = fixture();
    bad_symbols.symbols.push(FaslSymbol {
        id: 1,
        binding_kind: crate::fasl_symbols::SymbolBindingKind::Lisp,
        package: "CL".to_owned(),
        name: "CAR".to_owned(),
    });
    assert_eq!(
        FaslV1Writer::write(&bad_symbols),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );

    let mut bad_code = fixture();
    bad_code.code.push(FaslCodeObject {
        id: 0,
        entry_offset: 0,
        frame_words: 0,
        constant_count: 0,
        stack_map_count: 0,
        debug_record_count: 0,
        code: &[],
    });
    bad_code.constants.push(FaslConstantGroup {
        code_object_id: 0,
        constants: Vec::new(),
    });
    bad_code
        .stack_maps
        .push(crate::fasl_stack_map::SafepointGroup {
            code_object_id: 0,
            entries: Vec::new(),
        });
    assert_eq!(
        FaslV1Writer::write(&bad_code),
        Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
    );

    let mut bad_constants = fixture();
    bad_constants.code.push(FaslCodeObject {
        id: 0,
        entry_offset: 0,
        frame_words: 0,
        constant_count: 1,
        stack_map_count: 0,
        debug_record_count: 0,
        code: &[0x90],
    });
    bad_constants.constants.push(FaslConstantGroup {
        code_object_id: 0,
        constants: vec![crate::fasl_constants::FaslConstant::String(
            "x".repeat(1_048_577),
        )],
    });
    bad_constants
        .stack_maps
        .push(crate::fasl_stack_map::SafepointGroup {
            code_object_id: 0,
            entries: Vec::new(),
        });
    assert_eq!(
        FaslV1Writer::write(&bad_constants),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
}

#[test]
fn native_writer_rejects_noncanonical_stack_map_entries() {
    let mut bad_stack_map = fixture();
    bad_stack_map.code.push(FaslCodeObject {
        id: 0,
        entry_offset: 0,
        frame_words: 3,
        constant_count: 0,
        stack_map_count: 1,
        debug_record_count: 0,
        code: &[0x90],
    });
    bad_stack_map.constants.push(FaslConstantGroup {
        code_object_id: 0,
        constants: Vec::new(),
    });
    bad_stack_map
        .stack_maps
        .push(crate::fasl_stack_map::SafepointGroup {
            code_object_id: 0,
            entries: vec![crate::fasl_stack_map::SafepointEntry {
                pc_offset: 0,
                frame_words: 3,
                slot_words: 2,
                word_slot_count: 0,
                register_mask: 0,
                map_flags: 0,
                slot_bitmap: vec![0],
                register_ids: Vec::new(),
            }],
        });
    assert_eq!(
        FaslV1Writer::write(&bad_stack_map),
        Err(FaslError::new(FaslRejectReason::InvalidSafepointMap))
    );
}

#[test]
fn native_writer_propagates_debug_and_top_level_validation_errors() {
    let mut bad_debug = fixture();
    bad_debug.debug_files.push(crate::fasl_debug::DebugFile {
        file_id: 0,
        path: "../source.lisp".to_owned(),
    });
    assert_eq!(
        FaslV1Writer::write(&bad_debug),
        Err(FaslError::new(FaslRejectReason::InvalidDebugRecord))
    );

    let mut bad_top_level = fixture();
    bad_top_level.code.push(FaslCodeObject {
        id: 0,
        entry_offset: 0,
        frame_words: 0,
        constant_count: 0,
        stack_map_count: 0,
        debug_record_count: 0,
        code: &[0x90],
    });
    bad_top_level.constants.push(FaslConstantGroup {
        code_object_id: 0,
        constants: Vec::new(),
    });
    bad_top_level
        .stack_maps
        .push(crate::fasl_stack_map::SafepointGroup {
            code_object_id: 0,
            entries: Vec::new(),
        });
    bad_top_level
        .top_level
        .push(crate::fasl_top_level::FaslTopLevelRecord {
            ordinal: 0,
            code_object_id: 0,
            entry_offset: 0,
            flags: 4,
        });
    assert_eq!(
        FaslV1Writer::write(&bad_top_level),
        Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord))
    );
}

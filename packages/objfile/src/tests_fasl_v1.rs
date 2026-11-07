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

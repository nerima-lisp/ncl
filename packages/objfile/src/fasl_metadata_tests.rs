#[cfg(test)]
mod private_tests {
    use super::super::*;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const COMPILER_HASH: &str = "5fbe363777ce9bb412b2fbeacc5a04c50a7997723d41247bebb637c152741a66";
    const POLICY_HASH: &str = "4651527a57c9cdd6737015afa185561d09c08d9f9d4d19431286503966c710f8";
    const FEATURE_HASH: &str = "28f98dedf72165b0fcdf42ac52659504562676f98b31a0b03dc4444255898810";

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    fn hex(digest: [u8; 32]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in digest {
            output.push(char::from(DIGITS[usize::from(byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        output
    }

    fn metadata_fixture() -> FaslMetadata {
        FaslMetadata {
            semantic_version: "0.1.1".to_owned(),
            compiler_build_identity: [1; 32],
            codegen_abi_revision: 2,
            target_triple: "x86_64-unknown-linux-gnu".to_owned(),
            source_sha256: [4; 32],
            policies: vec![
                PolicyRecord { id: 1, value: 2 },
                PolicyRecord { id: 3, value: 0 },
            ],
            features: vec![FeatureRecord {
                name: "ncl-objfile/fasl-v1".to_owned(),
                version: "1".to_owned(),
            }],
        }
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn metadata_string(bytes: &[u8], base: usize) -> Result<String, FaslError> {
        Cursor::new(bytes, base).string()
    }

    #[test]
    fn metadata_round_trip_and_digests_match_canonical_vectors() {
        let metadata = metadata_fixture();
        let bytes = must_succeed(encode(&metadata));
        assert_eq!(decode(&bytes, 320), Ok(metadata.clone()));
        assert_eq!(hex(sha256(b"abc")), ABC_SHA256);
        let compiler = must_succeed(compiler_identity(&metadata));
        let policy = must_succeed(policy_fingerprint(&metadata.policies));
        let feature = must_succeed(feature_fingerprint(&metadata.features));
        assert_eq!(hex(compiler), COMPILER_HASH);
        assert_eq!(hex(policy), POLICY_HASH);
        assert_eq!(hex(feature), FEATURE_HASH);
    }

    #[test]
    fn metadata_rejects_noncanonical_policy_values() {
        let valid = metadata_fixture();
        let malformed = FaslError::at(FaslRejectReason::MalformedMetadata, 0);
        let invalid_policies = [
            ("zero policy ID", vec![PolicyRecord { id: 0, value: 0 }]),
            (
                "duplicate policy ID",
                vec![
                    PolicyRecord { id: 1, value: 0 },
                    PolicyRecord { id: 1, value: 1 },
                ],
            ),
            (
                "descending policy IDs",
                vec![
                    PolicyRecord { id: 3, value: 0 },
                    PolicyRecord { id: 1, value: 0 },
                ],
            ),
            (
                "policy value above three",
                vec![PolicyRecord { id: 1, value: 4 }],
            ),
        ];
        for (case, policies) in invalid_policies {
            let mut candidate = valid.clone();
            candidate.policies = policies;
            assert_eq!(validate(&candidate), Err(malformed), "{case}");
            assert_eq!(encode(&candidate), Err(malformed), "{case}");
        }
    }

    #[test]
    fn metadata_rejects_noncanonical_feature_and_string_values() {
        let valid = metadata_fixture();
        let malformed = FaslError::at(FaslRejectReason::MalformedMetadata, 0);
        let duplicate_feature = FeatureRecord {
            name: "ncl-objfile/fasl-v1".to_owned(),
            version: "1".to_owned(),
        };
        let invalid_features = [
            (
                "duplicate feature key",
                vec![duplicate_feature.clone(), duplicate_feature.clone()],
            ),
            (
                "descending versions for one feature",
                vec![
                    FeatureRecord {
                        name: duplicate_feature.name.clone(),
                        version: "2".to_owned(),
                    },
                    duplicate_feature,
                ],
            ),
            (
                "descending feature names",
                vec![
                    FeatureRecord {
                        name: "z-feature".to_owned(),
                        version: "1".to_owned(),
                    },
                    FeatureRecord {
                        name: "a-feature".to_owned(),
                        version: "1".to_owned(),
                    },
                ],
            ),
            (
                "empty feature name",
                vec![FeatureRecord {
                    name: String::new(),
                    version: "1".to_owned(),
                }],
            ),
            (
                "empty feature version",
                vec![FeatureRecord {
                    name: "feature".to_owned(),
                    version: String::new(),
                }],
            ),
            (
                "NUL in feature name",
                vec![FeatureRecord {
                    name: "feature\0name".to_owned(),
                    version: "1".to_owned(),
                }],
            ),
            (
                "NUL in feature version",
                vec![FeatureRecord {
                    name: "feature".to_owned(),
                    version: "1\0".to_owned(),
                }],
            ),
        ];
        for (case, features) in invalid_features {
            let mut candidate = valid.clone();
            candidate.features = features.clone();
            assert_eq!(validate(&candidate), Err(malformed), "{case}");
            assert_eq!(encode(&candidate), Err(malformed), "{case}");
            assert_eq!(feature_fingerprint(&features), Err(malformed), "{case}");
        }

        for (case, value) in [
            ("empty semantic version", String::new()),
            ("NUL in semantic version", "0.1\0.1".to_owned()),
        ] {
            let mut candidate = valid.clone();
            candidate.semantic_version = value;
            assert_eq!(validate(&candidate), Err(malformed), "{case}");
            assert_eq!(encode(&candidate), Err(malformed), "{case}");
        }
    }

    #[test]
    fn metadata_rejects_over_limit_count_and_string_length_headers() {
        let metadata = metadata_fixture();
        let valid = must_succeed(encode(&metadata));
        let policy_count_offset = 4
            + 4
            + metadata.semantic_version.len()
            + 32
            + 4
            + 4
            + metadata.target_triple.len()
            + 32;
        let feature_count_offset = policy_count_offset + 4 + metadata.policies.len() * 4;
        for count_offset in [policy_count_offset, feature_count_offset] {
            let mut oversized_count = valid.clone();
            write_u32(&mut oversized_count, count_offset, 1_048_577);
            assert_eq!(
                decode(&oversized_count, 320),
                Err(FaslError::at(
                    FaslRejectReason::ResourceLimitExceeded,
                    320 + count_offset
                ))
            );
        }

        let mut oversized_string = valid;
        write_u32(&mut oversized_string, 4, 1_048_577);
        assert_eq!(
            decode(&oversized_string, 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 324))
        );
    }

    #[test]
    fn metadata_string_cursor_rejects_truncated_invalid_and_empty_values() {
        let base = 400;
        let malformed_length = FaslError::at(FaslRejectReason::MalformedMetadata, base);
        assert_eq!(metadata_string(&[1, 0], base), Err(malformed_length));

        let malformed_payload = FaslError::at(FaslRejectReason::MalformedMetadata, base + 4);
        assert_eq!(
            metadata_string(&[2, 0, 0, 0, b'a'], base),
            Err(malformed_payload)
        );
        assert_eq!(metadata_string(&[0, 0, 0, 0], base), Err(malformed_payload));
        assert_eq!(
            metadata_string(&[1, 0, 0, 0, 0], base),
            Err(malformed_payload)
        );
        assert_eq!(
            metadata_string(&[1, 0, 0, 0, 0xff], base),
            Err(malformed_payload)
        );

        let too_long = must_succeed(u32::try_from(MAX_STRING_BYTES + 1));
        assert_eq!(
            metadata_string(&too_long.to_le_bytes(), base),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, base))
        );
    }
}

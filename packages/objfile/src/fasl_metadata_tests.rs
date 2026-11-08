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

    fn policy_count_offset(metadata: &FaslMetadata) -> usize {
        4 + 4 + metadata.semantic_version.len() + 32 + 4 + 4 + metadata.target_triple.len() + 32
    }

    fn feature_count_offset(metadata: &FaslMetadata) -> usize {
        policy_count_offset(metadata) + 4 + metadata.policies.len() * 4
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
    fn metadata_decoder_rejects_every_strict_prefix_of_a_valid_section() {
        let bytes = must_succeed(encode(&metadata_fixture()));
        for end in 0..bytes.len() {
            assert!(
                matches!(
                    decode(&bytes[..end], 320),
                    Err(FaslError {
                        reason: FaslRejectReason::MalformedMetadata,
                        ..
                    })
                ),
                "metadata accepted strict prefix ending at {end}"
            );
        }
    }

    #[test]
    fn metadata_size_limit_includes_the_exact_boundary() {
        assert!(!metadata_size_exceeds_limit(MAX_METADATA_BYTES));
        assert!(metadata_size_exceeds_limit(MAX_METADATA_BYTES + 1));
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

        for (case, value, reason) in [
            (
                "empty target triple",
                String::new(),
                FaslRejectReason::MalformedMetadata,
            ),
            (
                "NUL in target triple",
                "x86_64-unknown\0-linux-gnu".to_owned(),
                FaslRejectReason::MalformedMetadata,
            ),
            (
                "oversized target triple",
                "x".repeat(MAX_STRING_BYTES + 1),
                FaslRejectReason::ResourceLimitExceeded,
            ),
        ] {
            let mut candidate = valid.clone();
            candidate.target_triple = value;
            let expected = FaslError::at(reason, 0);
            assert_eq!(validate(&candidate), Err(expected), "{case}");
            assert_eq!(encode(&candidate), Err(expected), "{case}");
            assert_eq!(compiler_identity(&candidate), Err(expected), "{case}");
        }
    }

    #[test]
    fn metadata_rejects_over_limit_count_and_string_length_headers() {
        let metadata = metadata_fixture();
        let valid = must_succeed(encode(&metadata));
        for count_offset in [
            policy_count_offset(&metadata),
            feature_count_offset(&metadata),
        ] {
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
    fn metadata_decoder_rejects_invalid_wire_fields_and_incomplete_records() {
        let metadata = metadata_fixture();
        let valid = must_succeed(encode(&metadata));
        let policy_count = policy_count_offset(&metadata);

        let mut bad_schema = valid.clone();
        write_u32(&mut bad_schema, 0, 2);
        assert_eq!(
            decode(&bad_schema, 320),
            Err(FaslError::at(FaslRejectReason::MalformedMetadata, 320))
        );

        let mut bad_reserved = valid.clone();
        bad_reserved[policy_count + 4 + 3] = 1;
        assert_eq!(
            decode(&bad_reserved, 320),
            Err(FaslError::at(
                FaslRejectReason::MalformedMetadata,
                320 + policy_count + 4 + 3
            ))
        );

        let incomplete_policy_records = &valid[..policy_count + 4];
        assert_eq!(
            decode(incomplete_policy_records, 320),
            Err(FaslError::at(
                FaslRejectReason::MalformedMetadata,
                320 + policy_count + 4
            ))
        );

        let mut trailing = valid;
        trailing.push(0);
        assert_eq!(
            decode(&trailing, 320),
            Err(FaslError::at(
                FaslRejectReason::MalformedMetadata,
                320 + trailing.len() - 1
            ))
        );
    }

    #[test]
    fn metadata_decoder_revalidates_canonical_policy_and_feature_records() {
        let policy_metadata = metadata_fixture();
        let mut policy_bytes = must_succeed(encode(&policy_metadata));
        policy_bytes[policy_count_offset(&policy_metadata) + 4 + 2] = 4;
        assert_eq!(
            decode(&policy_bytes, 320),
            Err(FaslError::at(FaslRejectReason::MalformedMetadata, 320))
        );

        let mut feature_metadata = metadata_fixture();
        feature_metadata.features = vec![
            FeatureRecord {
                name: "a".to_owned(),
                version: "1".to_owned(),
            },
            FeatureRecord {
                name: "b".to_owned(),
                version: "1".to_owned(),
            },
        ];
        let mut feature_bytes = must_succeed(encode(&feature_metadata));
        let second_name = feature_count_offset(&feature_metadata) + 4 + 10 + 4;
        feature_bytes[second_name] = b'a';
        assert_eq!(
            decode(&feature_bytes, 320),
            Err(FaslError::at(FaslRejectReason::MalformedMetadata, 320))
        );
    }

    #[test]
    fn metadata_validation_enforces_string_and_record_limits() {
        let oversized = "x".repeat(MAX_STRING_BYTES + 1);
        let mut long_version = metadata_fixture();
        long_version.semantic_version = oversized.clone();
        let resource_limit = FaslError::at(FaslRejectReason::ResourceLimitExceeded, 0);
        assert_eq!(validate(&long_version), Err(resource_limit));
        assert_eq!(encode(&long_version), Err(resource_limit));
        assert_eq!(compiler_identity(&long_version), Err(resource_limit));

        let long_feature = [FeatureRecord {
            name: oversized,
            version: "1".to_owned(),
        }];
        assert_eq!(feature_fingerprint(&long_feature), Err(resource_limit));

        {
            let policies = vec![PolicyRecord { id: 1, value: 0 }; MAX_RECORDS + 1];
            let mut too_many = metadata_fixture();
            too_many.policies = policies.clone();
            assert_eq!(validate(&too_many), Err(resource_limit));
            assert_eq!(policy_fingerprint(&policies), Err(resource_limit));
        }
        {
            let features = vec![
                FeatureRecord {
                    name: String::new(),
                    version: String::new(),
                };
                MAX_RECORDS + 1
            ];
            let mut too_many = metadata_fixture();
            too_many.features = features.clone();
            assert_eq!(validate(&too_many), Err(resource_limit));
            assert_eq!(feature_fingerprint(&features), Err(resource_limit));
        }
    }

    #[test]
    fn sha256_matches_padding_boundary_vectors() {
        let empty: &[u8] = b"";
        let below_padding_boundary = [b'a'; 55];
        let at_padding_boundary = [b'a'; 56];
        let full_block = [b'a'; 64];
        for (input, expected) in [
            (
                empty,
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                &below_padding_boundary[..],
                "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
            ),
            (
                &at_padding_boundary[..],
                "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
            ),
            (
                &full_block[..],
                "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
            ),
        ] {
            assert_eq!(hex(sha256(input)), expected);
        }
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

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

    #[test]
    fn metadata_round_trip_and_digests_match_canonical_vectors() {
        let metadata = FaslMetadata {
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
        };
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
}

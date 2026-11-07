use crate::error::{FaslError, FaslRejectReason};

const METADATA_SCHEMA: u32 = 1;
const MAX_RECORDS: usize = 1_048_576;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_METADATA_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Compiler, target, source, and compatibility metadata stored in a FASL.
pub struct FaslMetadata {
    /// Cargo package semantic version recorded by the compiler.
    pub semantic_version: String,
    /// SHA-256 digest of the canonical compiler build manifest.
    pub compiler_build_identity: [u8; 32],
    /// Code-generation ABI revision used for emitted native code.
    pub codegen_abi_revision: u32,
    /// Target triple used to produce the native code.
    pub target_triple: String,
    /// SHA-256 digest of the original source bytes.
    pub source_sha256: [u8; 32],
    /// Canonically ordered compiler policy values.
    pub policies: Vec<PolicyRecord>,
    /// Canonically ordered required target features.
    pub features: Vec<FeatureRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One canonical policy identifier and level.
pub struct PolicyRecord {
    /// Policy identifier: 1 safety, 2 speed, or 3 debug.
    pub id: u16,
    /// Policy level from zero through three.
    pub value: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One canonical required target feature and version.
pub struct FeatureRecord {
    /// Crate-qualified feature name.
    pub name: String,
    /// Required feature version.
    pub version: String,
}

pub(super) fn encode(metadata: &FaslMetadata) -> Result<Vec<u8>, FaslError> {
    validate(metadata)?;
    let mut bytes = Vec::new();
    extend_bounded(&mut bytes, &METADATA_SCHEMA.to_le_bytes())?;
    append_string(&mut bytes, &metadata.semantic_version)?;
    extend_bounded(&mut bytes, &metadata.compiler_build_identity)?;
    extend_bounded(&mut bytes, &metadata.codegen_abi_revision.to_le_bytes())?;
    append_string(&mut bytes, &metadata.target_triple)?;
    extend_bounded(&mut bytes, &metadata.source_sha256)?;
    extend_bounded(
        &mut bytes,
        &count_u32(metadata.policies.len())?.to_le_bytes(),
    )?;
    for policy in &metadata.policies {
        extend_bounded(&mut bytes, &policy.id.to_le_bytes())?;
        extend_bounded(&mut bytes, &[policy.value, 0])?;
    }
    extend_bounded(
        &mut bytes,
        &count_u32(metadata.features.len())?.to_le_bytes(),
    )?;
    for feature in &metadata.features {
        append_string(&mut bytes, &feature.name)?;
        append_string(&mut bytes, &feature.version)?;
    }
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8], file_offset: usize) -> Result<FaslMetadata, FaslError> {
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            file_offset,
        ));
    }
    let mut cursor = Cursor::new(bytes, file_offset);
    let schema_offset = cursor.position;
    if cursor.u32()? != METADATA_SCHEMA {
        return Err(cursor.error_at(FaslRejectReason::MalformedMetadata, schema_offset));
    }
    let semantic_version = cursor.string()?;
    let compiler_build_identity = cursor.array::<32>()?;
    let codegen_abi_revision = cursor.u32()?;
    let target_triple = cursor.string()?;
    let source_sha256 = cursor.array::<32>()?;
    let policy_count = cursor.count(4)?;
    let mut policies = Vec::with_capacity(policy_count);
    for _ in 0..policy_count {
        let offset = cursor.position;
        let id = cursor.u16()?;
        let value = cursor.u8()?;
        if cursor.u8()? != 0 {
            return Err(cursor.error_at(FaslRejectReason::MalformedMetadata, offset + 3));
        }
        policies.push(PolicyRecord { id, value });
    }
    let feature_count = cursor.count(8)?;
    let mut features = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        features.push(FeatureRecord {
            name: cursor.string()?,
            version: cursor.string()?,
        });
    }
    if cursor.position != bytes.len() {
        return Err(cursor.error(FaslRejectReason::MalformedMetadata));
    }
    let metadata = FaslMetadata {
        semantic_version,
        compiler_build_identity,
        codegen_abi_revision,
        target_triple,
        source_sha256,
        policies,
        features,
    };
    validate_at(&metadata, file_offset)?;
    Ok(metadata)
}

pub(super) fn validate(metadata: &FaslMetadata) -> Result<(), FaslError> {
    validate_at(metadata, 0)
}

/// Computes the compiler identity digest from canonical FASL metadata.
///
/// # Errors
///
/// Returns `FaslError` when metadata strings or policy/feature records are invalid.
pub fn compiler_identity(metadata: &FaslMetadata) -> Result<[u8; 32], FaslError> {
    validate(metadata)?;
    let mut input = Vec::new();
    extend_bounded(&mut input, b"NCL-FASL-COMPILER-V1\0")?;
    append_string(&mut input, &metadata.semantic_version)?;
    extend_bounded(&mut input, &metadata.compiler_build_identity)?;
    extend_bounded(&mut input, &metadata.codegen_abi_revision.to_le_bytes())?;
    Ok(sha256(&input))
}

/// Computes the fingerprint of canonically ordered policy records.
///
/// # Errors
///
/// Returns `FaslError` when a policy ID, value, or order is invalid.
pub fn policy_fingerprint(policies: &[PolicyRecord]) -> Result<[u8; 32], FaslError> {
    validate_policies(policies, 0)?;
    let mut input = Vec::new();
    extend_bounded(&mut input, b"NCL-FASL-POLICY-V1\0")?;
    extend_bounded(&mut input, &count_u32(policies.len())?.to_le_bytes())?;
    for policy in policies {
        extend_bounded(&mut input, &policy.id.to_le_bytes())?;
        extend_bounded(&mut input, &[policy.value, 0])?;
    }
    Ok(sha256(&input))
}

/// Computes the fingerprint of canonically ordered required-feature records.
///
/// # Errors
///
/// Returns `FaslError` when a feature string or record order is invalid.
pub fn feature_fingerprint(features: &[FeatureRecord]) -> Result<[u8; 32], FaslError> {
    validate_features(features, 0)?;
    let mut input = Vec::new();
    extend_bounded(&mut input, b"NCL-FASL-FEATURES-V1\0")?;
    extend_bounded(&mut input, &count_u32(features.len())?.to_le_bytes())?;
    for feature in features {
        append_string(&mut input, &feature.name)?;
        append_string(&mut input, &feature.version)?;
    }
    Ok(sha256(&input))
}

fn validate_at(metadata: &FaslMetadata, base: usize) -> Result<(), FaslError> {
    validate_string(&metadata.semantic_version, base)?;
    validate_string(&metadata.target_triple, base)?;
    validate_policies(&metadata.policies, base)?;
    validate_features(&metadata.features, base)
}

fn validate_policies(policies: &[PolicyRecord], base: usize) -> Result<(), FaslError> {
    if policies.len() > MAX_RECORDS {
        return Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, base));
    }
    let mut previous = 0;
    for policy in policies {
        if !(1..=3).contains(&policy.id) || policy.id <= previous || policy.value > 3 {
            return Err(FaslError::at(FaslRejectReason::MalformedMetadata, base));
        }
        previous = policy.id;
    }
    Ok(())
}

fn validate_features(features: &[FeatureRecord], base: usize) -> Result<(), FaslError> {
    if features.len() > MAX_RECORDS {
        return Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, base));
    }
    let mut previous: Option<&FeatureRecord> = None;
    for feature in features {
        validate_string(&feature.name, base)?;
        validate_string(&feature.version, base)?;
        if previous.is_some_and(|prior| compare_features(prior, feature).is_ge()) {
            return Err(FaslError::at(FaslRejectReason::MalformedMetadata, base));
        }
        previous = Some(feature);
    }
    Ok(())
}

fn compare_features(left: &FeatureRecord, right: &FeatureRecord) -> core::cmp::Ordering {
    // With NUL-free names, comparing the two components is equivalent to
    // comparing the canonical `name || 0x00 || version` byte keys.
    left.name
        .as_bytes()
        .cmp(right.name.as_bytes())
        .then_with(|| left.version.as_bytes().cmp(right.version.as_bytes()))
}

fn validate_string(value: &str, offset: usize) -> Result<(), FaslError> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES {
        return Err(FaslError::at(
            if value.len() > MAX_STRING_BYTES {
                FaslRejectReason::ResourceLimitExceeded
            } else {
                FaslRejectReason::MalformedMetadata
            },
            offset,
        ));
    }
    if value.as_bytes().contains(&0) {
        return Err(FaslError::at(FaslRejectReason::MalformedMetadata, offset));
    }
    Ok(())
}

fn append_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), FaslError> {
    validate_string(value, 0)?;
    let length = u32::try_from(value.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    extend_bounded(bytes, &length.to_le_bytes())?;
    extend_bounded(bytes, value.as_bytes())?;
    Ok(())
}

fn extend_bounded(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), FaslError> {
    let end = bytes
        .len()
        .checked_add(value.len())
        .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    if end > MAX_METADATA_BYTES {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    bytes.extend_from_slice(value);
    Ok(())
}

fn count_u32(count: usize) -> Result<u32, FaslError> {
    if count > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let count = u32::try_from(count)
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    Ok(count)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    base: usize,
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], base: usize) -> Self {
        Self {
            bytes,
            base,
            position: 0,
        }
    }

    fn error(&self, reason: FaslRejectReason) -> FaslError {
        self.error_at(reason, self.position)
    }

    fn error_at(&self, reason: FaslRejectReason, position: usize) -> FaslError {
        let offset = self.base.saturating_add(position);
        FaslError::at(reason, offset)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], FaslError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error(FaslRejectReason::MalformedMetadata))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.error(FaslRejectReason::MalformedMetadata))?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], FaslError> {
        let source = self.take(N)?;
        let mut value = [0; N];
        value.copy_from_slice(source);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, FaslError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| self.error(FaslRejectReason::MalformedMetadata))
    }

    fn u16(&mut self) -> Result<u16, FaslError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    fn u32(&mut self) -> Result<u32, FaslError> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    fn count(&mut self, minimum_record_size: usize) -> Result<usize, FaslError> {
        let field_offset = self.position;
        let count = usize::try_from(self.u32()?)
            .map_err(|_| self.error_at(FaslRejectReason::ResourceLimitExceeded, field_offset))?;
        if count > MAX_RECORDS {
            return Err(self.error_at(FaslRejectReason::ResourceLimitExceeded, field_offset));
        }
        let minimum_bytes = count
            .checked_mul(minimum_record_size)
            .ok_or_else(|| self.error_at(FaslRejectReason::ResourceLimitExceeded, field_offset))?;
        if minimum_bytes > self.bytes.len().saturating_sub(self.position) {
            return Err(self.error(FaslRejectReason::MalformedMetadata));
        }
        Ok(count)
    }

    fn string(&mut self) -> Result<String, FaslError> {
        let length_offset = self.position;
        let length = usize::try_from(self.u32()?)
            .map_err(|_| self.error_at(FaslRejectReason::ResourceLimitExceeded, length_offset))?;
        if length > MAX_STRING_BYTES {
            return Err(self.error_at(FaslRejectReason::ResourceLimitExceeded, length_offset));
        }
        let string_offset = self.position;
        let raw = self.take(length)?;
        let value = core::str::from_utf8(raw)
            .map_err(|_| self.error_at(FaslRejectReason::MalformedMetadata, string_offset))?;
        if value.is_empty() || value.as_bytes().contains(&0) {
            return Err(self.error_at(FaslRejectReason::MalformedMetadata, string_offset));
        }
        Ok(value.to_owned())
    }
}

#[rustfmt::skip]
const SHA256_ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1,
    0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
    0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786,
    0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147,
    0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
    0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
    0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a,
    0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
    0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

/// Returns the dependency-free SHA-256 digest of the given bytes.
#[must_use]
pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut state = [
        0x6a09_e667u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    for chunk in input.chunks_exact(64) {
        let mut block = [0; 64];
        block.copy_from_slice(chunk);
        compress(&mut state, &block);
    }
    let remainder = input.chunks_exact(64).remainder();
    let bit_length = u64::try_from(input.len()).map_or(u64::MAX, |length| length.wrapping_mul(8));
    let mut final_block = [0; 64];
    final_block[..remainder.len()].copy_from_slice(remainder);
    final_block[remainder.len()] = 0x80;
    if remainder.len() >= 56 {
        compress(&mut state, &final_block);
        final_block = [0; 64];
    }
    final_block[56..64].copy_from_slice(&bit_length.to_be_bytes());
    compress(&mut state, &final_block);

    let mut digest = [0; 32];
    for (chunk, word) in digest.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    for (index, word) in block.chunks_exact(4).enumerate() {
        schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    for index in 16..64 {
        let x = schedule[index - 15];
        let y = schedule[index - 2];
        let sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
        let sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(sigma0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(sigma1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (word, constant) in schedule.into_iter().zip(SHA256_ROUND_CONSTANTS) {
        let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ (!e & g);
        let temp1 = h
            .wrapping_add(sigma1)
            .wrapping_add(choice)
            .wrapping_add(constant)
            .wrapping_add(word);
        let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = sigma0.wrapping_add(majority);
        [h, g, f, e, d, c, b, a] = [
            g,
            f,
            e,
            d.wrapping_add(temp1),
            c,
            b,
            a,
            temp1.wrapping_add(temp2),
        ];
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const COMPILER_HASH: &str = "5fbe363777ce9bb412b2fbeacc5a04c50a7997723d41247bebb637c152741a66";
    const POLICY_HASH: &str = "4651527a57c9cdd6737015afa185561d09c08d9f9d4d19431286503966c710f8";
    const FEATURE_HASH: &str = "28f98dedf72165b0fcdf42ac52659504562676f98b31a0b03dc4444255898810";

    fn hex(digest: [u8; 32]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
        let bytes = encode(&metadata).expect("valid metadata");
        assert_eq!(decode(&bytes, 320), Ok(metadata.clone()));
        assert_eq!(hex(sha256(b"abc")), ABC_SHA256);
        let compiler = compiler_identity(&metadata).expect("compiler identity");
        let policy = policy_fingerprint(&metadata.policies).expect("policy fingerprint");
        let feature = feature_fingerprint(&metadata.features).expect("feature fingerprint");
        assert_eq!(hex(compiler), COMPILER_HASH);
        assert_eq!(hex(policy), POLICY_HASH);
        assert_eq!(hex(feature), FEATURE_HASH);
    }
}

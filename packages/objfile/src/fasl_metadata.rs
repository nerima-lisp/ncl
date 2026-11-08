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

pub fn encode(metadata: &FaslMetadata) -> Result<Vec<u8>, FaslError> {
    validate(metadata)?;
    let mut bytes = Vec::with_capacity(encoded_size(metadata)?);
    bytes.extend_from_slice(&METADATA_SCHEMA.to_le_bytes());
    append_string(&mut bytes, &metadata.semantic_version);
    bytes.extend_from_slice(&metadata.compiler_build_identity);
    bytes.extend_from_slice(&metadata.codegen_abi_revision.to_le_bytes());
    append_string(&mut bytes, &metadata.target_triple);
    bytes.extend_from_slice(&metadata.source_sha256);
    bytes.extend_from_slice(&count_u32(metadata.policies.len()).to_le_bytes());
    for policy in &metadata.policies {
        bytes.extend_from_slice(&policy.id.to_le_bytes());
        bytes.extend_from_slice(&[policy.value, 0]);
    }
    bytes.extend_from_slice(&count_u32(metadata.features.len()).to_le_bytes());
    for feature in &metadata.features {
        append_string(&mut bytes, &feature.name);
        append_string(&mut bytes, &feature.version);
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8], file_offset: usize) -> Result<FaslMetadata, FaslError> {
    if metadata_size_exceeds_limit(bytes.len()) {
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

pub fn validate(metadata: &FaslMetadata) -> Result<(), FaslError> {
    validate_at(metadata, 0)
}

/// Computes the compiler identity digest from canonical FASL metadata.
///
/// # Errors
///
/// Returns `FaslError` when metadata strings or policy/feature records are invalid.
pub fn compiler_identity(metadata: &FaslMetadata) -> Result<[u8; 32], FaslError> {
    validate(metadata)?;
    let mut input = Vec::with_capacity(61 + metadata.semantic_version.len());
    input.extend_from_slice(b"NCL-FASL-COMPILER-V1\0");
    append_string(&mut input, &metadata.semantic_version);
    input.extend_from_slice(&metadata.compiler_build_identity);
    input.extend_from_slice(&metadata.codegen_abi_revision.to_le_bytes());
    Ok(sha256(&input))
}

/// Computes the fingerprint of canonically ordered policy records.
///
/// # Errors
///
/// Returns `FaslError` when a policy ID, value, or order is invalid.
pub fn policy_fingerprint(policies: &[PolicyRecord]) -> Result<[u8; 32], FaslError> {
    validate_policies(policies, 0)?;
    let mut input = Vec::with_capacity(21 + policies.len() * 6);
    input.extend_from_slice(b"NCL-FASL-POLICY-V1\0");
    input.extend_from_slice(&count_u32(policies.len()).to_le_bytes());
    for policy in policies {
        input.extend_from_slice(&policy.id.to_le_bytes());
        input.extend_from_slice(&[policy.value, 0]);
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
    let mut size = b"NCL-FASL-FEATURES-V1\0".len() + 4;
    for feature in features {
        size = bounded_size_add(size, 8 + feature.name.len() + feature.version.len())?;
    }
    let mut input = Vec::with_capacity(size);
    input.extend_from_slice(b"NCL-FASL-FEATURES-V1\0");
    input.extend_from_slice(&count_u32(features.len()).to_le_bytes());
    for feature in features {
        append_string(&mut input, &feature.name);
        append_string(&mut input, &feature.version);
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

#[expect(
    clippy::cast_possible_truncation,
    reason = "strings are validated before encoding"
)]
fn append_string(bytes: &mut Vec<u8>, value: &str) {
    // check-added-lines: allow(as-cast) the string length is bounded by MAX_STRING_BYTES.
    let length = value.len() as u32;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn encoded_size(metadata: &FaslMetadata) -> Result<usize, FaslError> {
    let mut size = 4;
    size = bounded_size_add(size, 4 + metadata.semantic_version.len())?;
    size = bounded_size_add(size, 32 + 4)?;
    size = bounded_size_add(size, 4 + metadata.target_triple.len())?;
    size = bounded_size_add(size, 32 + 4 + metadata.policies.len() * 4)?;
    size = bounded_size_add(size, 4)?;
    for feature in &metadata.features {
        size = bounded_size_add(size, 8 + feature.name.len() + feature.version.len())?;
    }
    Ok(size)
}

const fn bounded_size_add(current: usize, added: usize) -> Result<usize, FaslError> {
    if current > MAX_METADATA_BYTES || added > MAX_METADATA_BYTES - current {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(current + added)
}

const fn metadata_size_exceeds_limit(size: usize) -> bool {
    size > MAX_METADATA_BYTES
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "record counts are validated before encoding"
)]
const fn count_u32(count: usize) -> u32 {
    // check-added-lines: allow(as-cast) MAX_RECORDS is below u32::MAX.
    count as u32
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

    const fn error(&self, reason: FaslRejectReason) -> FaslError {
        self.error_at(reason, self.position)
    }

    const fn error_at(&self, reason: FaslRejectReason, position: usize) -> FaslError {
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
        let [value] = self.array::<1>()?;
        Ok(value)
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
        let minimum_bytes = count * minimum_record_size;
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
    let (blocks, remainder) = input.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }
    let bit_length = u64::try_from(input.len()).map_or(u64::MAX, |length| length.wrapping_mul(8));
    let mut final_block = [0; 64];
    final_block[..remainder.len()].copy_from_slice(remainder); // check-added-lines: allow(index) chunks_exact(64) guarantees a remainder shorter than the fixed block.
    final_block[remainder.len()] = 0x80; // check-added-lines: allow(index) the padding marker follows a remainder shorter than 64 bytes.
    if remainder.len() >= 56 {
        compress(&mut state, &final_block);
        final_block = [0; 64];
    }
    final_block[56..64].copy_from_slice(&bit_length.to_be_bytes()); // check-added-lines: allow(index) SHA-256 appends its length in the last 8 bytes of a 64-byte block.
    compress(&mut state, &final_block);

    let mut digest = [0; 32];
    let (chunks, _) = digest.as_chunks_mut::<4>();
    for (chunk, word) in chunks.iter_mut().zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    let (words, _) = block.as_chunks::<4>();
    for (slot, word) in schedule.iter_mut().zip(words) {
        *slot = u32::from_be_bytes(*word);
    }
    for index in 16..64 {
        let x = schedule[index - 15]; // check-added-lines: allow(index) index ranges from 16 to 63, so this reads initialized schedule words.
        let y = schedule[index - 2]; // check-added-lines: allow(index) index ranges from 16 to 63, so this reads initialized schedule words.
        let sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
        let sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
        schedule[index] = schedule[index - 16] // check-added-lines: allow(index) index ranges from 16 to 63, so all schedule positions are within the fixed 64-word array.
            .wrapping_add(sigma0)
            .wrapping_add(schedule[index - 7]) // check-added-lines: allow(index) index ranges from 16 to 63, so this reads an initialized schedule word.
            .wrapping_add(sigma1);
    }

    let [
        mut state_a,
        mut state_b,
        mut state_c,
        mut state_d,
        mut state_e,
        mut state_f,
        mut state_g,
        mut state_h,
    ] = *state;
    for (word, constant) in schedule.into_iter().zip(SHA256_ROUND_CONSTANTS) {
        let sigma1 = state_e.rotate_right(6) ^ state_e.rotate_right(11) ^ state_e.rotate_right(25);
        let choice = (state_e & state_f) ^ (!state_e & state_g);
        let temp1 = state_h
            .wrapping_add(sigma1)
            .wrapping_add(choice)
            .wrapping_add(constant)
            .wrapping_add(word);
        let sigma0 = state_a.rotate_right(2) ^ state_a.rotate_right(13) ^ state_a.rotate_right(22);
        let majority = (state_a & state_b) ^ (state_a & state_c) ^ (state_b & state_c);
        let temp2 = sigma0.wrapping_add(majority);
        [
            state_h, state_g, state_f, state_e, state_d, state_c, state_b, state_a,
        ] = [
            state_g,
            state_f,
            state_e,
            state_d.wrapping_add(temp1),
            state_c,
            state_b,
            state_a,
            temp1.wrapping_add(temp2),
        ];
    }
    for (slot, value) in state.iter_mut().zip([
        state_a, state_b, state_c, state_d, state_e, state_f, state_g, state_h,
    ]) {
        *slot = slot.wrapping_add(value);
    }
}
#[cfg(test)]
#[path = "fasl_metadata_tests.rs"]
mod tests;

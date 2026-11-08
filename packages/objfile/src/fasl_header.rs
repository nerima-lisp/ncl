use crate::error::{FaslError, FaslRejectReason};
use crate::fasl::Architecture;

pub const HEADER_SIZE: usize = 128;
const MAGIC: &[u8; 8] = b"NCLFASL\0";
const CONTAINER_VERSION: u16 = 3;
const SEMANTIC_VERSION: u32 = 1;
const SECTION_COUNT: u32 = 8;
const DIRECTORY_OFFSET: u32 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaslV1Header {
    pub architecture: Architecture,
    pub runtime_abi_version: u32,
    pub compiler_identity: [u8; 32],
    pub policy_fingerprint: [u8; 32],
    pub feature_fingerprint: [u8; 32],
}

pub fn encode(header: &FaslV1Header) -> [u8; HEADER_SIZE] {
    let mut bytes = [0; HEADER_SIZE];
    let fields = MAGIC
        .iter()
        .copied()
        .chain(CONTAINER_VERSION.to_le_bytes())
        .chain([header.architecture.wire_tag(), 8, 1, 128])
        .chain([0; 2])
        .chain(SEMANTIC_VERSION.to_le_bytes())
        .chain(header.runtime_abi_version.to_le_bytes())
        .chain(header.compiler_identity)
        .chain(header.policy_fingerprint)
        .chain(header.feature_fingerprint)
        .chain(SECTION_COUNT.to_le_bytes())
        .chain(DIRECTORY_OFFSET.to_le_bytes());
    for (destination, value) in bytes.iter_mut().zip(fields) {
        *destination = value;
    }
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<FaslV1Header, FaslError> {
    if bytes.len() < HEADER_SIZE {
        return Err(FaslError::at(
            FaslRejectReason::MalformedDirectory,
            bytes.len(),
        ));
    }
    if bytes.get(..8) != Some(MAGIC.as_slice()) {
        return Err(FaslError::at(FaslRejectReason::BadMagic, 0));
    }
    if u16::from_le_bytes(fixed_field::<2>(bytes, 8)) != CONTAINER_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 8));
    }
    if u32::from_le_bytes(fixed_field::<4>(bytes, 16)) != SEMANTIC_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 16));
    }

    let architecture = match u8::from_le_bytes(fixed_field::<1>(bytes, 10)) {
        1 => Architecture::X86_64,
        2 => Architecture::Aarch64,
        0 | 3..=u8::MAX => {
            return Err(FaslError::at(FaslRejectReason::TargetMismatch, 10));
        }
    };
    if u8::from_le_bytes(fixed_field::<1>(bytes, 11)) != 8 {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 11));
    }
    if u8::from_le_bytes(fixed_field::<1>(bytes, 12)) != 1 {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 12));
    }
    if u8::from_le_bytes(fixed_field::<1>(bytes, 13)) != 128 {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 13));
    }
    if u16::from_le_bytes(fixed_field::<2>(bytes, 14)) != 0 {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14));
    }
    if u32::from_le_bytes(fixed_field::<4>(bytes, 120)) != SECTION_COUNT {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 120));
    }
    if u32::from_le_bytes(fixed_field::<4>(bytes, 124)) != DIRECTORY_OFFSET {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 124));
    }

    Ok(FaslV1Header {
        architecture,
        runtime_abi_version: u32::from_le_bytes(fixed_field::<4>(bytes, 20)),
        compiler_identity: fixed_field(bytes, 24),
        policy_fingerprint: fixed_field(bytes, 56),
        feature_fingerprint: fixed_field(bytes, 88),
    })
}

fn fixed_field<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let end = offset + N;
    let mut field = [0; N];
    // check-added-lines: allow(index) decode validates 128 bytes before fixed header reads.
    field.copy_from_slice(&bytes[offset..end]);
    field
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], FaslError> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    let source = bytes
        .get(offset..end)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    let mut value = [0; N];
    value.copy_from_slice(source);
    Ok(value)
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, FaslError> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, FaslError> {
    read_array(bytes, offset).map(u16::from_le_bytes)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, FaslError> {
    read_array(bytes, offset).map(u32::from_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> FaslV1Header {
        FaslV1Header {
            architecture: Architecture::Aarch64,
            runtime_abi_version: 7,
            compiler_identity: [1; 32],
            policy_fingerprint: [2; 32],
            feature_fingerprint: [3; 32],
        }
    }

    fn write_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> bool {
        let Some(end) = offset.checked_add(value.len()) else {
            return false;
        };
        let Some(destination) = bytes.get_mut(offset..end) else {
            return false;
        };
        destination.copy_from_slice(value);
        true
    }

    #[test]
    fn v1_header_matches_all_fixed_offsets_and_round_trips() {
        let encoded = encode(&header());
        assert_eq!(encoded.get(..8), Some(b"NCLFASL\0".as_slice()));
        assert_eq!(read_u16(&encoded, 8), Ok(3));
        assert_eq!(encoded.get(10..14), Some([2, 8, 1, 128].as_slice()));
        assert_eq!(read_u16(&encoded, 14), Ok(0));
        assert_eq!(read_u32(&encoded, 16), Ok(1));
        assert_eq!(read_u32(&encoded, 20), Ok(7));
        assert_eq!(encoded.get(24..56), Some([1; 32].as_slice()));
        assert_eq!(encoded.get(56..88), Some([2; 32].as_slice()));
        assert_eq!(encoded.get(88..120), Some([3; 32].as_slice()));
        assert_eq!(read_u32(&encoded, 120), Ok(8));
        assert_eq!(read_u32(&encoded, 124), Ok(128));
        assert_eq!(decode(&encoded), Ok(header()));

        let mut x86_header = header();
        x86_header.architecture = Architecture::X86_64;
        let x86_encoded = encode(&x86_header);
        assert_eq!(x86_encoded.get(10), Some(&1));
        assert_eq!(decode(&x86_encoded), Ok(x86_header));
    }

    #[test]
    fn v1_header_rejects_magic_version_and_target_fields_by_reason() {
        let valid = encode(&header());
        let mut bad_magic = valid;
        assert!(write_bytes(&mut bad_magic, 0, b"X"));
        assert_eq!(
            decode(&bad_magic).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::BadMagic, 0))
        );

        let mut bad_version = valid;
        assert!(write_bytes(&mut bad_version, 8, &2u16.to_le_bytes()));
        assert_eq!(
            decode(&bad_version).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 8))
        );

        let mut bad_architecture = valid;
        assert!(write_bytes(&mut bad_architecture, 10, &[9]));
        assert_eq!(
            decode(&bad_architecture).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 10))
        );

        let mut bad_reserved = valid;
        assert!(write_bytes(&mut bad_reserved, 14, &[1]));
        assert_eq!(
            decode(&bad_reserved).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14))
        );
        let mut bad_semantic_version = valid;
        assert!(write_bytes(
            &mut bad_semantic_version,
            16,
            &2u32.to_le_bytes()
        ));
        assert_eq!(
            decode(&bad_semantic_version).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 16))
        );
        let mut bad_pointer_width = valid;
        assert!(write_bytes(&mut bad_pointer_width, 11, &[4]));
        assert_eq!(
            decode(&bad_pointer_width).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 11))
        );
        let mut bad_endianness = valid;
        assert!(write_bytes(&mut bad_endianness, 12, &[2]));
        assert_eq!(
            decode(&bad_endianness).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 12))
        );
        let mut bad_section_count = valid;
        assert!(write_bytes(
            &mut bad_section_count,
            120,
            &7u32.to_le_bytes()
        ));
        assert_eq!(
            decode(&bad_section_count).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 120))
        );
        assert_eq!(
            decode(valid.get(..127).unwrap_or(&[])),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 127))
        );
    }

    #[test]
    fn v1_header_rejects_remaining_fixed_fields_and_reserved_bytes() {
        let valid = encode(&header());

        let mut bad_reserved_high_byte = valid;
        assert!(write_bytes(&mut bad_reserved_high_byte, 15, &[1]));
        assert_eq!(
            decode(&bad_reserved_high_byte),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14))
        );

        let mut bad_data_representation = valid;
        assert!(write_bytes(&mut bad_data_representation, 13, &[0]));
        assert_eq!(
            decode(&bad_data_representation),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 13))
        );

        let mut bad_directory_offset = valid;
        assert!(write_bytes(
            &mut bad_directory_offset,
            124,
            &129_u32.to_le_bytes()
        ));
        assert_eq!(
            decode(&bad_directory_offset),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 124))
        );
    }

    #[test]
    fn header_field_readers_reject_overflow_and_short_ranges() {
        let malformed = FaslError::at(FaslRejectReason::MalformedDirectory, 7);
        assert_eq!(read_u8(&[], 7), Err(malformed));
        assert_eq!(read_u32(&[], 7), Err(malformed));
        assert_eq!(read_array::<2>(&[], 7), Err(malformed));
        assert_eq!(read_array::<32>(&[], 7), Err(malformed));
        assert_eq!(
            read_array::<2>(&[], usize::MAX),
            Err(FaslError::at(
                FaslRejectReason::MalformedDirectory,
                usize::MAX,
            ))
        );
    }
}

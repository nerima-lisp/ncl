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
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&CONTAINER_VERSION.to_le_bytes());
    bytes[10] = header.architecture.wire_tag();
    bytes[11..14].copy_from_slice(&[8, 1, 128]);
    bytes[16..20].copy_from_slice(&SEMANTIC_VERSION.to_le_bytes());
    bytes[20..24].copy_from_slice(&header.runtime_abi_version.to_le_bytes());
    bytes[24..56].copy_from_slice(&header.compiler_identity);
    bytes[56..88].copy_from_slice(&header.policy_fingerprint);
    bytes[88..120].copy_from_slice(&header.feature_fingerprint);
    bytes[120..124].copy_from_slice(&SECTION_COUNT.to_le_bytes());
    bytes[124..128].copy_from_slice(&DIRECTORY_OFFSET.to_le_bytes());
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<FaslV1Header, FaslError> {
    if bytes.len() < HEADER_SIZE {
        return Err(FaslError::at(
            FaslRejectReason::MalformedDirectory,
            bytes.len(),
        ));
    }
    if &bytes[..8] != MAGIC {
        return Err(FaslError::at(FaslRejectReason::BadMagic, 0));
    }
    if read_u16(bytes, 8) != CONTAINER_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 8));
    }
    if read_u32(bytes, 16) != SEMANTIC_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 16));
    }

    let architecture = match bytes[10] {
        1 => Architecture::X86_64,
        2 => Architecture::Aarch64,
        0 | 3..=u8::MAX => {
            return Err(FaslError::at(FaslRejectReason::TargetMismatch, 10));
        }
    };
    if bytes[11] != 8 {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 11));
    }
    if bytes[12] != 1 {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 12));
    }
    if bytes[13] != 128 {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 13));
    }
    if read_u16(bytes, 14) != 0 {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14));
    }
    if read_u32(bytes, 120) != SECTION_COUNT {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 120));
    }
    if read_u32(bytes, 124) != DIRECTORY_OFFSET {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 124));
    }

    Ok(FaslV1Header {
        architecture,
        runtime_abi_version: read_u32(bytes, 20),
        compiler_identity: read_array(bytes, 24),
        policy_fingerprint: read_array(bytes, 56),
        feature_fingerprint: read_array(bytes, 88),
    })
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut value = [0; N];
    value.copy_from_slice(&bytes[offset..offset + N]);
    value
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(read_array(bytes, offset))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array(bytes, offset))
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

    #[test]
    fn v1_header_matches_all_fixed_offsets_and_round_trips() {
        let encoded = encode(&header());
        assert_eq!(&encoded[..8], b"NCLFASL\0");
        assert_eq!(u16::from_le_bytes([encoded[8], encoded[9]]), 3);
        assert_eq!(encoded[10..14], [2, 8, 1, 128]);
        assert_eq!(u16::from_le_bytes([encoded[14], encoded[15]]), 0);
        assert_eq!(u32::from_le_bytes(read_array::<4>(&encoded, 16)), 1);
        assert_eq!(u32::from_le_bytes(read_array::<4>(&encoded, 20)), 7);
        assert_eq!(&encoded[24..56], &[1; 32]);
        assert_eq!(&encoded[56..88], &[2; 32]);
        assert_eq!(&encoded[88..120], &[3; 32]);
        assert_eq!(u32::from_le_bytes(read_array::<4>(&encoded, 120)), 8);
        assert_eq!(u32::from_le_bytes(read_array::<4>(&encoded, 124)), 128);
        assert_eq!(decode(&encoded), Ok(header()));

        let mut x86_header = header();
        x86_header.architecture = Architecture::X86_64;
        let x86_encoded = encode(&x86_header);
        assert_eq!(x86_encoded[10], 1);
        assert_eq!(decode(&x86_encoded), Ok(x86_header));
    }

    #[test]
    fn v1_header_rejects_magic_version_and_target_fields_by_reason() {
        let valid = encode(&header());
        let mut bad_magic = valid;
        bad_magic[0] = b'X';
        assert_eq!(
            decode(&bad_magic).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::BadMagic, 0))
        );

        let mut bad_version = valid;
        bad_version[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            decode(&bad_version).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 8))
        );

        let mut bad_architecture = valid;
        bad_architecture[10] = 9;
        assert_eq!(
            decode(&bad_architecture).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 10))
        );

        let mut bad_reserved = valid;
        bad_reserved[14] = 1;
        assert_eq!(
            decode(&bad_reserved).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14))
        );
        let mut bad_semantic_version = valid;
        bad_semantic_version[16..20].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            decode(&bad_semantic_version).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 16))
        );
        let mut bad_pointer_width = valid;
        bad_pointer_width[11] = 4;
        assert_eq!(
            decode(&bad_pointer_width).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 11))
        );
        let mut bad_endianness = valid;
        bad_endianness[12] = 2;
        assert_eq!(
            decode(&bad_endianness).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::TargetMismatch, 12))
        );
        let mut bad_section_count = valid;
        bad_section_count[120..124].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(
            decode(&bad_section_count).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 120))
        );
        assert_eq!(
            decode(&valid[..127]),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 127))
        );
    }

    #[test]
    fn v1_header_rejects_remaining_fixed_fields_and_reserved_bytes() {
        let valid = encode(&header());

        let mut bad_reserved_high_byte = valid;
        bad_reserved_high_byte[15] = 1;
        assert_eq!(
            decode(&bad_reserved_high_byte),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14))
        );

        let mut bad_data_representation = valid;
        bad_data_representation[13] = 0;
        assert_eq!(
            decode(&bad_data_representation),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 13))
        );

        let mut bad_directory_offset = valid;
        bad_directory_offset[124..128].copy_from_slice(&129_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_directory_offset),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 124))
        );
    }
}

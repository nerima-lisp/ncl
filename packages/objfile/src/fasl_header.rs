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

pub fn encode(header: &FaslV1Header) -> Result<[u8; HEADER_SIZE], FaslError> {
    let mut bytes = [0; HEADER_SIZE];
    write_range(&mut bytes, 0, MAGIC)?;
    write_range(&mut bytes, 8, &CONTAINER_VERSION.to_le_bytes())?;
    write_range(&mut bytes, 10, &[header.architecture.wire_tag()])?;
    write_range(&mut bytes, 11, &[8, 1, 128])?;
    write_range(&mut bytes, 16, &SEMANTIC_VERSION.to_le_bytes())?;
    write_range(&mut bytes, 20, &header.runtime_abi_version.to_le_bytes())?;
    write_range(&mut bytes, 24, &header.compiler_identity)?;
    write_range(&mut bytes, 56, &header.policy_fingerprint)?;
    write_range(&mut bytes, 88, &header.feature_fingerprint)?;
    write_range(&mut bytes, 120, &SECTION_COUNT.to_le_bytes())?;
    write_range(&mut bytes, 124, &DIRECTORY_OFFSET.to_le_bytes())?;
    Ok(bytes)
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
    if read_u16(bytes, 8)? != CONTAINER_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 8));
    }
    if read_u32(bytes, 16)? != SEMANTIC_VERSION {
        return Err(FaslError::at(FaslRejectReason::UnsupportedVersion, 16));
    }

    let architecture = match bytes.get(10).copied() {
        Some(1) => Architecture::X86_64,
        Some(2) => Architecture::Aarch64,
        Some(0 | 3..=u8::MAX) | None => {
            return Err(FaslError::at(FaslRejectReason::TargetMismatch, 10));
        }
    };
    if bytes.get(11) != Some(&8) {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 11));
    }
    if bytes.get(12) != Some(&1) {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 12));
    }
    if bytes.get(13) != Some(&128) {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 13));
    }
    if read_u16(bytes, 14)? != 0 {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 14));
    }
    if read_u32(bytes, 120)? != SECTION_COUNT {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 120));
    }
    if read_u32(bytes, 124)? != DIRECTORY_OFFSET {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 124));
    }

    Ok(FaslV1Header {
        architecture,
        runtime_abi_version: read_u32(bytes, 20)?,
        compiler_identity: read_array(bytes, 24)?,
        policy_fingerprint: read_array(bytes, 56)?,
        feature_fingerprint: read_array(bytes, 88)?,
    })
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

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, FaslError> {
    let value = read_array::<2>(bytes, offset)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, FaslError> {
    let value = read_array::<4>(bytes, offset)?;
    Ok(u32::from_le_bytes(value))
}

fn write_range(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), FaslError> {
    let end = offset
        .checked_add(value.len())
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    let destination = bytes
        .get_mut(offset..end)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    destination.copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

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
        let encoded = must_succeed(encode(&header()));
        assert_eq!(&encoded[..8], b"NCLFASL\0");
        assert_eq!(u16::from_le_bytes([encoded[8], encoded[9]]), 3);
        assert_eq!(encoded[10..14], [2, 8, 1, 128]);
        assert_eq!(u16::from_le_bytes([encoded[14], encoded[15]]), 0);
        assert_eq!(
            u32::from_le_bytes(must_succeed(read_array::<4>(&encoded, 16))),
            1
        );
        assert_eq!(
            u32::from_le_bytes(must_succeed(read_array::<4>(&encoded, 20))),
            7
        );
        assert_eq!(&encoded[24..56], &[1; 32]);
        assert_eq!(&encoded[56..88], &[2; 32]);
        assert_eq!(&encoded[88..120], &[3; 32]);
        assert_eq!(
            u32::from_le_bytes(must_succeed(read_array::<4>(&encoded, 120))),
            8
        );
        assert_eq!(
            u32::from_le_bytes(must_succeed(read_array::<4>(&encoded, 124))),
            128
        );
        assert_eq!(decode(&encoded), Ok(header()));
    }

    #[test]
    fn v1_header_rejects_magic_version_and_target_fields_by_reason() {
        let valid = must_succeed(encode(&header()));
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
}

use super::*;

#[test]
fn safepoint_decode_rejects_register_ids_outside_the_supported_range() {
    let mut bytes = vec![0; 16];
    bytes[4..6].copy_from_slice(&4_u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&3_u16.to_le_bytes());
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes.push(0b0000_0100);
    bytes.extend_from_slice(&16_u16.to_le_bytes());

    assert_eq!(
        SafepointMap::decode(&bytes, 1),
        Err(SafepointMapError::InvalidRegisterId)
    );
}

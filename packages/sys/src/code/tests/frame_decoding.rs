use super::*;

fn map_entry(
    slots: u16,
    word_slots: u16,
    register_mask: u16,
    bitmap: Option<u8>,
    register_ids: &[u16],
) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes[4..6].copy_from_slice(&4_u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&slots.to_le_bytes());
    bytes[8..10].copy_from_slice(&word_slots.to_le_bytes());
    bytes[10..12].copy_from_slice(&register_mask.to_le_bytes());
    if let Some(bitmap) = bitmap {
        bytes.push(bitmap);
    }
    for register_id in register_ids {
        bytes.extend_from_slice(&register_id.to_le_bytes());
    }
    bytes
}

#[test]
fn safepoint_decode_reports_typed_errors_for_malformed_entries() {
    let cases = [
        (
            "truncated header",
            vec![0; 15],
            SafepointMapError::TruncatedHeader,
        ),
        (
            "too few slots",
            map_entry(2, 2, 0, None, &[]),
            SafepointMapError::InvalidSlotCounts,
        ),
        (
            "more word slots than slots",
            map_entry(3, 4, 0, None, &[]),
            SafepointMapError::InvalidSlotCounts,
        ),
        (
            "truncated slot bitmap",
            map_entry(3, 3, 0, None, &[]),
            SafepointMapError::TruncatedSlotBitmap,
        ),
        (
            "missing function slot in header bitmap",
            map_entry(3, 3, 0, Some(0), &[]),
            SafepointMapError::InvalidHeaderBitmap,
        ),
        (
            "reserved header slot marked live",
            map_entry(3, 3, 0, Some(0b0000_0110), &[]),
            SafepointMapError::InvalidHeaderBitmap,
        ),
        (
            "truncated register ids",
            map_entry(3, 3, 1, Some(0b0000_0100), &[]),
            SafepointMapError::TruncatedRegisterIds,
        ),
        (
            "register id outside the supported range",
            map_entry(3, 3, 1, Some(0b0000_0100), &[16]),
            SafepointMapError::InvalidRegisterId,
        ),
        (
            "register id outside the mask",
            map_entry(3, 3, 1, Some(0b0000_0100), &[3]),
            SafepointMapError::RegisterMaskMismatch,
        ),
        (
            "duplicate register id",
            map_entry(3, 3, 0b0011, Some(0b0000_0100), &[0, 0]),
            SafepointMapError::RegisterMaskMismatch,
        ),
    ];

    for (case, bytes, expected) in cases {
        // check-added-lines: allow(panic) test-only assertions
        assert_eq!(SafepointMap::decode(&bytes, 1), Err(expected), "{case}");
    }
}

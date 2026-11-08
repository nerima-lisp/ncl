use super::*;

#[test]
fn constant_payload_readers_reject_truncated_tag_fields() {
    let cases: &[(u16, &[u8], usize)] = &[
        (3, &[], 340),
        (4, &[], 340),
        (5, &[], 340),
        (7, &[], 340),
        (8, &[], 340),
        (8, &[0; 4], 344),
        (11, &[], 340),
        (11, &[0; 4], 344),
        (12, &[], 340),
        (12, &[0; 4], 344),
        (10, &[], 340),
        (10, &[0], 341),
    ];

    for (tag, payload, offset) in cases {
        assert_eq!(
            decode(&raw_constant(*tag, 0, payload), 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, *offset)),
            "constant tag {tag} with payload length {}",
            payload.len()
        );
    }
}

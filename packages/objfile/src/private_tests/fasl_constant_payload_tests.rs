use crate::fasl_constants::decode;
use crate::fasl_records::SectionWriter;
use crate::{FaslError, FaslRejectReason};

fn raw_constant(tag: u16, flags: u16, payload_length: u32, payload: &[u8]) -> Vec<u8> {
    let mut writer = SectionWriter::new();
    writer.put_u32(1);
    writer.put_u32(0);
    writer.put_u32(1);
    writer.put_u16(tag);
    writer.put_u16(flags);
    writer.put_u32(payload_length);
    writer.put_bytes(payload);
    writer.zero_padding(4);
    writer.into_bytes()
}

#[test]
fn constant_payload_readers_reject_truncated_tag_fields() {
    let cases: &[(u16, u32, &[u8], usize)] = &[
        (3, 0, &[], 340),
        (4, 0, &[], 340),
        (5, 0, &[], 340),
        (7, 0, &[], 340),
        (8, 0, &[], 340),
        (8, 4, &[0; 4], 344),
        (11, 0, &[], 340),
        (11, 4, &[0; 4], 344),
        (12, 0, &[], 340),
        (12, 4, &[0; 4], 344),
        (10, 0, &[], 340),
        (10, 1, &[0], 341),
    ];

    for (tag, payload_length, payload, offset) in cases {
        assert_eq!(
            decode(&raw_constant(*tag, 0, *payload_length, payload), 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, *offset)),
            "constant tag {tag} with payload length {}",
            payload.len()
        );
    }
}

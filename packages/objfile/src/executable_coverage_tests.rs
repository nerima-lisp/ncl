use super::*;

#[test]
fn private_elf_segment_parser_skips_non_load_segments() {
    let mut bytes = vec![0; 56];
    bytes[0..4].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        validate_elf_segments(&bytes, 0, 56, 1, 0),
        Ok((false, false))
    );
}

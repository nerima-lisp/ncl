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

#[test]
fn private_elf_header_parser_rejects_truncated_and_wrong_class_inputs() {
    assert_eq!(
        validate_elf_executable_header(&[0; 63], Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 executable",
        ))
    );
    let mut bytes = vec![0; 64];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable_header(&bytes, Architecture::X86_64),
        Ok(())
    );
    bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable_header(&bytes, Architecture::X86_64),
        Err(ObjectError::InvalidStructure("not an ELF executable"))
    );
}

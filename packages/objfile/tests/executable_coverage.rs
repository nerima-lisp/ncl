//! Executable envelope validation coverage.
#![allow(clippy::expect_used, clippy::redundant_clone)]

use ncl_objfile::*;

fn image(architecture: Architecture) -> ExecutableImage {
    ExecutableImage {
        architecture,
        code: vec![0xc3, 0x90],
        metadata: vec![1, 2],
    }
}

#[test]
fn elf_executable_rejects_header_and_segment_metadata_errors() {
    let valid = write_elf_executable(&image(Architecture::X86_64)).expect("valid executable");
    let mut bad = valid.clone();
    bad[0] = 0;
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 executable",
        ))
    );

    let mut bad = valid.clone();
    bad[4] = 1;
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 executable",
        ))
    );
    let mut bad = valid.clone();
    bad[16..18].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidStructure("not an ELF executable"))
    );
    let mut bad = valid.clone();
    bad[18..20].copy_from_slice(&183u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "ELF machine",
            value: 183,
        })
    );

    let mut bad = valid.clone();
    bad[64..68].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );
    let mut bad = valid.clone();
    bad[68..72].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );
    let mut bad = valid.clone();
    bad[64 + 32..64 + 40].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_elf_executable(&bad, Architecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "ELF load segment",
            ..
        })
    ));
}

#[test]
fn elf_executable_checks_entry_and_architecture_variants() {
    let arm = image(Architecture::Aarch64);
    let bytes = write_elf_executable(&arm).expect("AArch64 executable");
    assert_eq!(
        validate_elf_executable(&bytes, Architecture::Aarch64),
        Ok(())
    );
    let mut bad_entry = bytes;
    bad_entry[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bad_entry, Architecture::Aarch64),
        Err(ObjectError::InvalidStructure(
            "ELF entry is outside executable segment",
        ))
    );
}

#[test]
fn elf_executable_requires_rx_text_and_rw_metadata_segments() {
    let valid = write_elf_executable(&image(Architecture::X86_64)).expect("valid executable");

    let mut non_load = valid.clone();
    non_load[64..68].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&non_load, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );

    let mut non_executable_text = valid.clone();
    non_executable_text[68..72].copy_from_slice(&6u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&non_executable_text, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );

    let mut non_writable_metadata = valid;
    non_writable_metadata[64 + 56 + 4..64 + 56 + 8].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&non_writable_metadata, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );
}

#[test]
fn mach_executable_requires_main_and_nonempty_metadata() {
    let x86_image = image(Architecture::X86_64);
    let valid =
        write_mach_executable(&x86_image, MachArchitecture::X86_64).expect("Mach executable");
    assert_eq!(
        validate_mach_executable(&valid, MachArchitecture::X86_64),
        Ok(())
    );

    let mut no_main = valid.clone();
    no_main[400..404].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_mach_executable(&no_main, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable metadata"
        ))
    );

    let mut no_metadata = valid;
    no_metadata[296..304].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(
        validate_mach_executable(&no_metadata, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable metadata"
        ))
    );

    assert_eq!(
        write_mach_executable(&image(Architecture::Aarch64), MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "architecture",
            value: 2,
        })
    );
}

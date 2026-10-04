use super::*;
use crate::{Architecture, ExecutableImage, write_mach_executable};

#[test]
fn private_macho_relocation_encoder_covers_descriptor_bits() {
    let kinds = [
        (MachArchitecture::X86_64, crate::RelocKind::Abs64),
        (MachArchitecture::Arm64, crate::RelocKind::Abs64),
        (MachArchitecture::Arm64, crate::RelocKind::Adrp21),
        (MachArchitecture::Arm64, crate::RelocKind::Add12),
        (MachArchitecture::Arm64, crate::RelocKind::Branch26),
        (MachArchitecture::X86_64, crate::RelocKind::Plt32),
        (MachArchitecture::X86_64, crate::RelocKind::PcRel32),
    ];
    for (architecture, kind) in kinds {
        let relocation = Relocation {
            section: SectionId(1),
            offset: 4,
            kind,
            symbol: crate::SymbolRef::External("loader".into()),
            addend: 0,
        };
        assert!(encode_relocation(&relocation, architecture).is_ok());
    }
    let unsupported = Relocation {
        section: SectionId(1),
        offset: 0,
        kind: crate::RelocKind::CondBranch19,
        symbol: crate::SymbolRef::Local(0),
        addend: 0,
    };
    assert_eq!(
        encode_relocation(&unsupported, MachArchitecture::Arm64),
        Err(ObjectError::UnsupportedRelocation(
            crate::RelocKind::CondBranch19
        ))
    );
}

#[test]
fn private_macho_name_and_input_validation_rejects_invalid_data() {
    let valid = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![1],
        }],
        relocations: vec![],
    };
    assert_eq!(validate_input(&valid), Ok(()));
    let mut invalid_name = valid.clone();
    invalid_name.sections[0].segment = "0123456789abcdefx".into();
    assert_eq!(validate_input(&invalid_name), Err(ObjectError::InvalidName));
    let mut invalid_kind = valid;
    invalid_kind.relocations.push(Relocation {
        section: SectionId(1),
        offset: 0,
        kind: crate::RelocKind::CondBranch19,
        symbol: crate::SymbolRef::Local(0),
        addend: 0,
    });
    assert_eq!(
        validate_input(&invalid_kind),
        Err(ObjectError::UnsupportedRelocation(
            crate::RelocKind::CondBranch19
        ))
    );
}

#[test]
fn private_macho_readers_report_truncation() {
    assert_eq!(
        read_u32(&[1, 2, 3], 0, "command"),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 4,
        })
    );
    assert_eq!(
        read_u64(&[1, 2, 3], 0, "section size"),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 8,
        })
    );
    assert_eq!(
        validate_macho_commands(&[0; 32], 36, 1),
        Err(ObjectError::Truncated {
            offset: 32,
            needed: 4,
        })
    );
    let mut command = vec![0; 36];
    command[32..36].copy_from_slice(&0x19u32.to_le_bytes());
    assert_eq!(
        validate_macho_commands(&command, 36, 1),
        Err(ObjectError::Truncated {
            offset: 36,
            needed: 4,
        })
    );
}

#[test]
fn private_macho_command_parser_reports_missing_and_bad_sizes() {
    let mut command = vec![0; 40];
    command[32..36].copy_from_slice(&0u32.to_le_bytes());
    command[36..40].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        validate_macho_commands(&command, 40, 1),
        Err(ObjectError::InvalidStructure("missing Mach-O load command"))
    );

    command[36..40].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(
        validate_macho_commands(&command, 40, 1),
        Err(ObjectError::InvalidStructure("invalid Mach-O command size"))
    );
}

#[test]
fn private_macho_command_parser_checks_section_field_bounds() {
    let mut command = vec![0; 155];
    command[32..36].copy_from_slice(&0x19u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    command[36..40].copy_from_slice(&152u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    command[96..100].copy_from_slice(&1u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho_commands(&command, 184, 1),
        Err(ObjectError::Truncated {
            offset: 152,
            needed: 4,
        })
    );

    let mut section = vec![0; 155];
    section[32..36].copy_from_slice(&0x19u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    section[36..40].copy_from_slice(&152u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    section[96..100].copy_from_slice(&1u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho_commands(&section, 184, 1),
        Err(ObjectError::Truncated {
            offset: 152,
            needed: 4,
        })
    );
}

#[test]
fn private_macho_command_parser_reports_truncated_command_fields() {
    let mut command = vec![0; 36];
    command[32..36].copy_from_slice(&0x19u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(matches!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho_commands(&command, 40, 1),
        Err(ObjectError::Truncated {
            offset: 36,
            needed: 4
        })
    ));

    let mut section_count = vec![0; 99];
    section_count[32..36].copy_from_slice(&0x19u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    section_count[36..40].copy_from_slice(&72u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(matches!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho_commands(&section_count, 104, 1),
        Err(ObjectError::Truncated {
            offset: 96,
            needed: 4
        })
    ));
}

#[test]
fn private_macho_command_parser_rejects_non_segment_commands() {
    let mut command = vec![0; 40];
    command[32..36].copy_from_slice(&0x2u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    command[36..40].copy_from_slice(&8u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho_commands(&command, 40, 1),
        Err(ObjectError::InvalidStructure("missing Mach-O load command"))
    );
}

#[test]
fn public_macho_writer_and_reader_cover_reachable_architecture_paths() {
    for (architecture, kind) in [
        (MachArchitecture::X86_64, crate::RelocKind::PcRel32),
        (MachArchitecture::Arm64, crate::RelocKind::Branch26),
    ] {
        let value = MachObject {
            architecture,
            sections: vec![MachSection {
                id: SectionId(1),
                segment: "__TEXT".into(),
                name: "__text".into(),
                bytes: vec![0; 8],
            }],
            relocations: vec![Relocation {
                section: SectionId(1),
                offset: 0,
                kind,
                symbol: crate::SymbolRef::Local(0),
                addend: 0,
            }],
        };
        let bytes_result = value.write();
        assert!(bytes_result.is_ok()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        let bytes = bytes_result.unwrap_or_default();
        assert_eq!(MachReader::validate(&bytes, architecture), Ok(())); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    }
}

#[test]
fn public_macho_validator_reports_header_and_command_errors() {
    // check-added-lines: allow(panic,index,as-cast) test fixture assertion
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho(&[], MachArchitecture::X86_64),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 32
        })
    );
    let mut bytes = vec![0; 32];
    bytes[0..4].copy_from_slice(&0xfeed_facf_u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[4..8].copy_from_slice(&0x0100_0007u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[16..20].copy_from_slice(&1u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[20..24].copy_from_slice(&8u32.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands"
        ))
    );
    bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(matches!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands"
        ))
    ));
}

#[test]
fn public_macho_validator_reports_cpu_and_command_boundaries() {
    let value = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![1],
        }],
        relocations: vec![],
    };
    let bytes = value.write().unwrap_or_default();
    let mut bad_cpu = bytes.clone();
    bad_cpu[4..8].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_macho(&bad_cpu, MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "Mach-O CPU",
            value: 0
        })
    );
    let mut no_commands = bytes.clone();
    no_commands[16..20].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_macho(&no_commands, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands"
        ))
    );
    let mut bad_size = bytes;
    bad_size[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        validate_macho(&bad_size, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid Mach-O command size"))
    );
}

#[test]
fn mach_executable_validator_rejects_missing_main_and_metadata() {
    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![1],
        metadata: vec![2],
    };
    let bytes = write_mach_executable(&image, MachArchitecture::X86_64).unwrap_or_default();
    let mut missing_main = bytes.clone();
    missing_main[400..404].copy_from_slice(&0x2u32.to_le_bytes());
    assert_eq!(
        validate_mach_executable(&missing_main, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable metadata"
        ))
    );
    let mut empty_metadata = bytes;
    empty_metadata[32 + 152 + 72 + 40..32 + 152 + 72 + 48].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(
        validate_mach_executable(&empty_metadata, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable metadata"
        ))
    );
}

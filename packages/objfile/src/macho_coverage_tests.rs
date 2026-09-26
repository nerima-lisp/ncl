use super::*;

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

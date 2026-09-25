//! Coverage tests for Mach-O writer paths.
#![allow(clippy::expect_used, clippy::redundant_clone)]

use ncl_objfile::*;

#[test]
fn macho_writer_rejects_invalid_names_and_references() {
    let base = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![0xc3],
        }],
        relocations: vec![],
    };
    let long_name = MachObject {
        sections: vec![MachSection {
            name: "0123456789abcdefx".into(),
            ..base.sections[0].clone()
        }],
        ..base.clone()
    };
    assert_eq!(long_name.write(), Err(ObjectError::InvalidName));
    let invalid_reference = MachObject {
        relocations: vec![Relocation {
            section: SectionId(9),
            offset: 0,
            kind: RelocKind::Abs64,
            symbol: SymbolRef::Local(0),
            addend: 0,
        }],
        ..base
    };
    assert_eq!(
        invalid_reference.write(),
        Err(ObjectError::InvalidReference {
            kind: "section",
            index: 9
        })
    );
}

#[test]
fn macho_writer_covers_supported_x86_and_arm_relocations() {
    let section = MachSection {
        id: SectionId(1),
        segment: "__TEXT".into(),
        name: "__text".into(),
        bytes: vec![0; 16],
    };
    let x86 = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![section.clone()],
        relocations: vec![
            Relocation {
                section: SectionId(1),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: 0,
            },
            Relocation {
                section: SectionId(1),
                offset: 1,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::External("external".into()),
                addend: 0,
            },
            Relocation {
                section: SectionId(1),
                offset: 2,
                kind: RelocKind::PcRel32,
                symbol: SymbolRef::External("external".into()),
                addend: 0,
            },
            Relocation {
                section: SectionId(1),
                offset: 3,
                kind: RelocKind::Plt32,
                symbol: SymbolRef::External("external".into()),
                addend: 0,
            },
        ],
    };
    assert_eq!(
        validate_macho(&x86.write().unwrap_or_default(), MachArchitecture::X86_64),
        Ok(())
    );
    let arm = MachObject {
        architecture: MachArchitecture::Arm64,
        sections: vec![section],
        relocations: [
            RelocKind::Abs64,
            RelocKind::Branch26,
            RelocKind::Adrp21,
            RelocKind::Add12,
        ]
        .iter()
        .enumerate()
        .map(|(index, kind)| Relocation {
            section: SectionId(1),
            offset: u32::try_from(index).unwrap_or_default(),
            kind: *kind,
            symbol: SymbolRef::External("external".into()),
            addend: 0,
        })
        .collect(),
    };
    assert_eq!(
        validate_macho(&arm.write().unwrap_or_default(), MachArchitecture::Arm64),
        Ok(())
    );
}

#[test]
fn macho_reader_rejects_short_segments_and_section_tables() {
    let object = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![0xc3],
        }],
        relocations: vec![],
    };
    let valid = object.write().expect("valid Mach-O");
    assert_eq!(
        MachReader::validate(&valid, MachArchitecture::X86_64),
        Ok(())
    );

    let mut short_segment = valid.clone();
    short_segment[36..40].copy_from_slice(&64u32.to_le_bytes());
    assert_eq!(
        validate_macho(&short_segment, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("short LC_SEGMENT_64"))
    );

    let mut short_table = valid.clone();
    short_table[96..100].copy_from_slice(&1u32.to_le_bytes());
    short_table[36..40].copy_from_slice(&72u32.to_le_bytes());
    assert_eq!(
        validate_macho(&short_table, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("short Mach-O section table"))
    );

    let mut table_overflow = valid.clone();
    table_overflow[96..100].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        validate_macho(&table_overflow, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("short Mach-O section table"))
    );

    let mut invalid_command_end = valid;
    invalid_command_end[20..24].copy_from_slice(&7u32.to_le_bytes());
    invalid_command_end[36..40].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        validate_macho(&invalid_command_end, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid Mach-O command size"))
    );
}

#[test]
fn macho_validator_rejects_missing_segment_and_bad_section_range() {
    let object = MachObject {
        architecture: MachArchitecture::Arm64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![0xc3],
        }],
        relocations: vec![],
    };
    let valid = object.write().expect("valid Mach-O");
    let mut no_segment = valid.clone();
    no_segment[32..36].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        validate_macho(&no_segment, MachArchitecture::Arm64),
        Err(ObjectError::InvalidStructure("missing Mach-O load command"))
    );

    let mut out_of_bounds = valid;
    out_of_bounds[152..160].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_macho(&out_of_bounds, MachArchitecture::Arm64),
        Err(ObjectError::OutOfBounds {
            section: "Mach-O section",
            ..
        })
    ));
}

#[test]
fn macho_validation_exercises_truncated_commands_and_unsupported_relocations() {
    let mut header = vec![0; 40];
    header[0..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
    header[4..8].copy_from_slice(&0x0100_0007u32.to_le_bytes());
    header[16..20].copy_from_slice(&1u32.to_le_bytes());
    header[20..24].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        validate_macho(&header, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid Mach-O command size"))
    );

    let section = MachSection {
        id: SectionId(1),
        segment: "__TEXT".into(),
        name: "__text".into(),
        bytes: vec![0xc3],
    };
    let unsupported = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![section.clone()],
        relocations: vec![Relocation {
            section: SectionId(1),
            offset: 0,
            kind: RelocKind::Branch26,
            symbol: SymbolRef::Local(0),
            addend: 0,
        }],
    };
    assert_eq!(
        unsupported.write(),
        Err(ObjectError::UnsupportedRelocation(RelocKind::Branch26))
    );

    let invalid_kind = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![section],
        relocations: vec![Relocation {
            section: SectionId(1),
            offset: 0,
            kind: RelocKind::CondBranch19,
            symbol: SymbolRef::Local(0),
            addend: 0,
        }],
    };
    assert_eq!(
        invalid_kind.write(),
        Err(ObjectError::UnsupportedRelocation(RelocKind::CondBranch19))
    );
}

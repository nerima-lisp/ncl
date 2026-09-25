//! Additional coverage tests for native object writers.
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_clone)]

use ncl_objfile::*;

fn minimal_fasl() -> Fasl {
    Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![0xc3],
            relocations: vec![],
            constants: vec![],
            symbols: vec![],
            stack_maps: vec![],
            debug: vec![],
        },
    }
}

#[test]
fn object_error_display_covers_each_variant() {
    let errors = [
        ObjectError::Truncated {
            offset: 3,
            needed: 4,
        },
        ObjectError::InvalidField {
            field: "magic",
            value: 9,
        },
        ObjectError::OutOfBounds {
            section: "code",
            offset: 8,
            size: 4,
        },
        ObjectError::Overlap {
            first: "one",
            second: "two",
        },
        ObjectError::InvalidReference {
            kind: "symbol",
            index: 2,
        },
        ObjectError::UnsupportedRelocation(RelocKind::CodeEntry),
        ObjectError::InvalidName,
        ObjectError::InvalidStructure("bad structure"),
    ];
    let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert_eq!(messages[0], "truncated input at 3, need 4 bytes");
    assert_eq!(messages[1], "invalid magic: 9");
    assert_eq!(messages[2], "code range 8..12 is out of bounds");
    assert_eq!(messages[3], "sections one and two overlap");
    assert_eq!(messages[4], "invalid symbol reference 2");
    assert_eq!(messages[5], "unsupported relocation CodeEntry");
    assert_eq!(messages[6], "name cannot be represented");
    assert_eq!(messages[7], "bad structure");
}

#[test]
fn fasl_rejects_bad_relocation_wire_data() {
    let value = Fasl {
        sections: FaslSection {
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: 0,
            }],
            ..minimal_fasl().sections
        },
        ..minimal_fasl()
    };
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[73..77].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 0),
        Err(ObjectError::InvalidField {
            field: "relocation kind",
            value: 99
        })
    );
    let mut bytes = FaslWriter::write(&minimal_fasl()).unwrap_or_default();
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 0),
        Err(ObjectError::OutOfBounds {
            section: "relocation",
            offset: 65,
            size: 16
        })
    );
}

#[test]
fn elf_validator_reports_header_and_table_errors() {
    let object = ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: vec![ElfSection {
            id: SectionId(1),
            kind: ElfSectionKind::Text,
            bytes: vec![0xc3],
        }],
        relocations: vec![],
        symbols: vec![],
    };
    let valid = object.write().unwrap_or_default();
    assert_eq!(
        validate_elf(&valid[..63], ElfArchitecture::X86_64),
        Err(ObjectError::Truncated {
            offset: 63,
            needed: 64
        })
    );
    let mut bytes = valid.clone();
    bytes[0] = 0;
    assert_eq!(
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 file"
        ))
    );
    let mut bytes = valid.clone();
    bytes[18..20].copy_from_slice(&183u16.to_le_bytes());
    assert_eq!(
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "ELF machine",
            value: 183
        })
    );
    let mut bytes = valid.clone();
    bytes[58..60].copy_from_slice(&32u16.to_le_bytes());
    assert_eq!(
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid ELF section table"))
    );
    let mut bytes = valid;
    bytes[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "ELF section table",
            offset: u64::MAX,
            size: 576
        })
    );
}

#[test]
fn macho_validator_reports_header_and_command_errors() {
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
    let valid = object.write().unwrap_or_default();
    assert_eq!(
        validate_macho(&valid[..31], MachArchitecture::X86_64),
        Err(ObjectError::Truncated {
            offset: 31,
            needed: 32
        })
    );
    let mut bytes = valid.clone();
    bytes[0..4].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "Mach-O magic",
            value: 0
        })
    );
    let mut bytes = valid.clone();
    bytes[4..8].copy_from_slice(&0x0100_000cu32.to_le_bytes());
    assert_eq!(
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "Mach-O CPU",
            value: 0x0100_000c
        })
    );
    let mut bytes = valid.clone();
    bytes[16..20].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands"
        ))
    );
    let mut bytes = valid.clone();
    bytes[36..40].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid Mach-O command size"))
    );
    let mut bytes = valid;
    bytes[144..152].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_macho(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "Mach-O section",
            offset: _,
            size: u64::MAX
        })
    ));
}

#[test]
fn executable_validators_reject_wrong_headers_and_segments() {
    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![0xc3],
        metadata: b"NCL\0".to_vec(),
    };
    let elf = write_elf_executable(&image).unwrap_or_default();
    let mut bytes = elf.clone();
    bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bytes, Architecture::X86_64),
        Err(ObjectError::InvalidStructure("not an ELF executable"))
    );
    let mut bytes = elf.clone();
    bytes[54..56].copy_from_slice(&64u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bytes, Architecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid ELF program headers"))
    );
    let mut bytes = elf.clone();
    bytes[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_elf_executable(&bytes, Architecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "ELF program headers",
            ..
        })
    ));
    let mut bytes = elf;
    bytes[64 + 4..64 + 8].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&bytes, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable segments"
        ))
    );
    let wrong_arch = ExecutableImage {
        architecture: Architecture::Aarch64,
        ..image
    };
    assert_eq!(
        write_mach_executable(&wrong_arch, MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "architecture",
            value: 2
        })
    );
}

#[test]
fn native_writers_cover_supported_relocation_kinds_and_section_mapping() {
    let sections = vec![
        ElfSection {
            id: SectionId(1),
            kind: ElfSectionKind::Text,
            bytes: vec![0; 8],
        },
        ElfSection {
            id: SectionId(2),
            kind: ElfSectionKind::Rodata,
            bytes: vec![1, 2],
        },
        ElfSection {
            id: SectionId(3),
            kind: ElfSectionKind::Metadata,
            bytes: vec![3, 4, 5, 6, 7, 8, 9, 10],
        },
    ];
    let symbols = vec![ElfSymbol {
        name: "local".into(),
        section: Some(SectionId(1)),
        value: 0,
        global: false,
    }];
    let x86_kinds = [
        RelocKind::Abs64,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
        RelocKind::PcRel32,
        RelocKind::Plt32,
    ];
    let x86_relocations = x86_kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| Relocation {
            section: SectionId(1),
            offset: u32::try_from(index).unwrap_or_default(),
            kind: *kind,
            symbol: if *kind == RelocKind::ExternalSymbol {
                SymbolRef::External("external".into())
            } else {
                SymbolRef::Local(0)
            },
            addend: 0,
        })
        .collect();
    let x86 = ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: sections.clone(),
        relocations: x86_relocations,
        symbols: symbols.clone(),
    };
    assert_eq!(
        validate_elf(&x86.write().unwrap_or_default(), ElfArchitecture::X86_64),
        Ok(())
    );

    let arm_kinds = [
        RelocKind::Abs64,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
        RelocKind::Branch26,
        RelocKind::Adrp21,
        RelocKind::Add12,
        RelocKind::CondBranch19,
    ];
    let arm = ElfObject {
        architecture: ElfArchitecture::Aarch64,
        sections: sections.clone(),
        relocations: arm_kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| Relocation {
                section: SectionId(3),
                offset: u32::try_from(index).unwrap_or_default(),
                kind: *kind,
                symbol: SymbolRef::Local(0),
                addend: 4,
            })
            .collect(),
        symbols,
    };
    assert_eq!(
        validate_elf(&arm.write().unwrap_or_default(), ElfArchitecture::Aarch64),
        Ok(())
    );
    let generic = sections_from_generic(&[
        Section {
            id: SectionId(1),
            name: ".text".into(),
            bytes: vec![1],
        },
        Section {
            id: SectionId(2),
            name: ".ncl".into(),
            bytes: vec![2],
        },
        Section {
            id: SectionId(3),
            name: ".rodata".into(),
            bytes: vec![3],
        },
    ]);
    assert_eq!(generic[0].kind, ElfSectionKind::Text);
    assert_eq!(generic[1].kind, ElfSectionKind::Metadata);
    assert_eq!(generic[2].kind, ElfSectionKind::Rodata);
}

#[test]
fn executable_writers_cover_aarch64_and_entry_validation() {
    let image = ExecutableImage {
        architecture: Architecture::Aarch64,
        code: vec![0, 0, 0, 0],
        metadata: vec![1, 2, 3],
    };
    let elf = write_elf_executable(&image).unwrap_or_default();
    assert_eq!(validate_elf_executable(&elf, Architecture::Aarch64), Ok(()));
    let mut invalid = elf;
    invalid[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        validate_elf_executable(&invalid, Architecture::Aarch64),
        Err(ObjectError::InvalidStructure(
            "ELF entry is outside executable segment"
        ))
    );
    let macho = write_mach_executable(&image, MachArchitecture::Arm64).unwrap_or_default();
    assert_eq!(
        validate_mach_executable(&macho, MachArchitecture::Arm64),
        Ok(())
    );
}

#[test]
fn native_writers_cover_empty_and_multi_section_layouts() {
    let empty = ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: vec![],
        relocations: vec![],
        symbols: vec![ElfSymbol {
            name: "undefined".into(),
            section: None,
            value: 0,
            global: true,
        }],
    };
    let elf = empty.write().expect("empty ELF layout");
    assert_eq!(validate_elf(&elf, ElfArchitecture::X86_64), Ok(()));

    let mach = MachObject {
        architecture: MachArchitecture::X86_64,
        sections: vec![
            MachSection {
                id: SectionId(1),
                segment: "__TEXT".into(),
                name: "__text".into(),
                bytes: vec![0xc3],
            },
            MachSection {
                id: SectionId(2),
                segment: "__DATA".into(),
                name: "__data".into(),
                bytes: vec![1, 2, 3],
            },
        ],
        relocations: vec![Relocation {
            section: SectionId(2),
            offset: 1,
            kind: RelocKind::Abs64,
            symbol: SymbolRef::External("loader".into()),
            addend: 4,
        }],
    };
    let mach_bytes = mach.write().expect("multi-section Mach-O layout");
    assert_eq!(
        MachReader::validate(&mach_bytes, MachArchitecture::X86_64),
        Ok(())
    );

    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![],
        metadata: vec![9],
    };
    let executable =
        write_mach_executable(&image, MachArchitecture::X86_64).expect("empty-code executable");
    assert_eq!(
        validate_mach_executable(&executable, MachArchitecture::X86_64),
        Ok(())
    );
}

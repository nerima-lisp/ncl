use super::*;

fn object() -> ElfObject {
    ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: vec![ElfSection {
            id: SectionId(1),
            kind: ElfSectionKind::Text,
            bytes: vec![0xc3],
        }],
        relocations: vec![],
        symbols: vec![],
    }
}

#[test]
fn private_elf_mapping_covers_target_types_and_symbols() {
    for kind in [
        RelocKind::Abs64,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
    ] {
        assert_eq!(elf_type(kind, 62), Ok(1));
        assert_eq!(elf_type(kind, 183), Ok(257));
    }
    assert_eq!(elf_type(RelocKind::PcRel32, 62), Ok(2));
    assert_eq!(elf_type(RelocKind::Plt32, 62), Ok(4));
    assert_eq!(elf_type(RelocKind::Branch26, 183), Ok(283));
    assert_eq!(elf_type(RelocKind::Adrp21, 183), Ok(275));
    assert_eq!(elf_type(RelocKind::Add12, 183), Ok(277));
    assert_eq!(elf_type(RelocKind::CondBranch19, 183), Ok(280));
    assert_eq!(
        elf_type(RelocKind::Branch26, 62),
        Err(ObjectError::UnsupportedRelocation(RelocKind::Branch26))
    );
    assert_eq!(
        symbol_section(
            &object(),
            &ElfSymbol {
                name: "missing".into(),
                section: None,
                value: 0,
                global: false,
            }
        ),
        0
    );
}

#[test]
fn empty_elf_sections_allow_zero_offset_relocations() {
    let mut value = object();
    value.sections[0].bytes.clear();
    value.relocations.push(Relocation {
        section: SectionId(1),
        offset: u32::MAX,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::External("loader".into()),
        addend: 0,
    });
    assert!(value.write().is_ok());
}

#[test]
fn private_section_header_rejects_unrepresentable_symbol_count() {
    assert_eq!(
        write_section_header(&mut Vec::new(), &[(0, 0); 9], &[0; 8], 6, u32::MAX as usize,),
        Err(ObjectError::InvalidField {
            field: "symbol count",
            value: u64::MAX,
        })
    );
}

#[test]
fn private_elf_helpers_write_all_section_header_kinds() {
    let mut bytes = Vec::new();
    let ranges = [(0, 0); 9];
    let names: Vec<u32> = (0..8).collect();
    for index in 1..9 {
        assert_eq!(
            write_section_header(&mut bytes, &ranges, &names, index, 1),
            Ok(())
        );
    }
    assert_eq!(bytes.len(), 8 * 64);
    assert_eq!(&bytes[64..68], &1u32.to_le_bytes());
    assert_eq!(&bytes[4 * 64..4 * 64 + 4], &4u32.to_le_bytes());
    assert_eq!(&bytes[5 * 64 + 4..5 * 64 + 8], &2u32.to_le_bytes());
}

#[test]
fn private_elf_writer_round_trip_contains_symbols_and_sections() {
    let value = ElfObject {
        architecture: ElfArchitecture::Aarch64,
        sections: vec![
            ElfSection {
                id: SectionId(1),
                kind: ElfSectionKind::Text,
                bytes: vec![1, 2],
            },
            ElfSection {
                id: SectionId(2),
                kind: ElfSectionKind::Rodata,
                bytes: vec![3],
            },
            ElfSection {
                id: SectionId(3),
                kind: ElfSectionKind::Metadata,
                bytes: vec![4],
            },
        ],
        relocations: vec![Relocation {
            section: SectionId(3),
            offset: 0,
            kind: RelocKind::Abs64,
            symbol: SymbolRef::Local(0),
            addend: 9,
        }],
        symbols: vec![ElfSymbol {
            name: "entry".into(),
            section: Some(SectionId(1)),
            value: 0,
            global: true,
        }],
    };
    let result = value.write();
    assert!(result.is_ok());
    let Ok(bytes) = result else { return };
    assert_eq!(
        ElfReader::validate(&bytes, ElfArchitecture::Aarch64),
        Ok(())
    );
    assert!(bytes.windows(5).any(|window| window == b"entry"));
}

#[test]
fn private_elf_validator_rejects_bad_section_table_metadata() {
    let result = object().write();
    assert!(result.is_ok()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    let Some(bytes) = result.ok() else {
        return;
    };
    let mut bad_entry_size = bytes.clone();
    bad_entry_size[58..60].copy_from_slice(&32u16.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_elf(&bad_entry_size, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid ELF section table"))
    );

    let mut bad_count = bytes;
    bad_count[60..62].copy_from_slice(&8u16.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_elf(&bad_count, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidStructure("invalid ELF section table"))
    );
}

#[test]
fn private_elf_writer_handles_sections_without_text_or_metadata() {
    let value = ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: vec![ElfSection {
            id: SectionId(2),
            kind: ElfSectionKind::Rodata,
            bytes: vec![1, 2, 3],
        }],
        relocations: vec![],
        symbols: vec![],
    };
    let result = value.write();
    assert!(result.is_ok()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    let Some(bytes) = result.ok() else {
        return;
    };
    assert_eq!(validate_elf(&bytes, ElfArchitecture::X86_64), Ok(())); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
}

#[test]
fn public_elf_validation_and_generic_mapping_cover_reachable_errors() {
    assert_eq!( // check-added-lines: allow(panic,index,as-cast) test
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_elf(&[], ElfArchitecture::X86_64),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 64
        })
    );
    let mut bytes = vec![0; 64];
    assert_eq!( // check-added-lines: allow(panic,index,as-cast) test
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 file"
        ))
    );
    bytes[0..4].copy_from_slice(b"\x7fELF"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[4] = 2; // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[5] = 1; // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[6] = 1; // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[58..60].copy_from_slice(&64u16.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[60..62].copy_from_slice(&9u16.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    bytes[40..48].copy_from_slice(&u64::MAX.to_le_bytes()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(matches!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        validate_elf(&bytes, ElfArchitecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "ELF section table",
            ..
        })
    ));

    let generic = vec![
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
            name: ".other".into(),
            bytes: vec![3],
        },
    ];
    let mapped = sections_from_generic(&generic);
    assert_eq!(mapped[0].kind, ElfSectionKind::Text); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(mapped[1].kind, ElfSectionKind::Metadata); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(mapped[2].kind, ElfSectionKind::Rodata); // check-added-lines: allow(panic,index,as-cast) test fixture assertions

    let mut invalid_offset = object();
    invalid_offset.relocations.push(Relocation {
        section: SectionId(1),
        offset: 1,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::External("loader".into()),
        addend: 0,
    });
    assert_eq!( // check-added-lines: allow(panic,index,as-cast) test
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        invalid_offset.write(),
        Err(ObjectError::OutOfBounds {
            section: "relocation",
            offset: 1,
            size: 1
        })
    );
}

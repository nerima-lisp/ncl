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
    let bytes = value.write().expect("ELF with all sections");
    assert_eq!(
        ElfReader::validate(&bytes, ElfArchitecture::Aarch64),
        Ok(())
    );
    assert!(bytes.windows(5).any(|window| window == b"entry"));
}

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
        write_section_header(
            &mut Vec::new(),
            &[(0, 0); 9],
            &[0; 8],
            6,
            u32::MAX as usize,
        ),
        Err(ObjectError::InvalidField {
            field: "symbol count",
            value: u64::MAX,
        })
    );
}

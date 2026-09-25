//! ELF writer and validator coverage.
#![allow(clippy::expect_used, clippy::redundant_clone)]

use ncl_objfile::*;

fn object() -> ElfObject {
    ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: vec![ElfSection {
            id: SectionId(1),
            kind: ElfSectionKind::Text,
            bytes: vec![0xc3; 8],
        }],
        relocations: vec![],
        symbols: vec![ElfSymbol {
            name: "entry".into(),
            section: Some(SectionId(1)),
            value: 0,
            global: true,
        }],
    }
}

#[test]
fn elf_input_validation_rejects_bad_relocations() {
    let mut invalid_section = object();
    invalid_section.relocations.push(Relocation {
        section: SectionId(9),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: 0,
    });
    assert_eq!(
        invalid_section.write(),
        Err(ObjectError::InvalidReference {
            kind: "section",
            index: 9,
        })
    );

    let mut invalid_symbol = object();
    invalid_symbol.relocations.push(Relocation {
        section: SectionId(1),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(9),
        addend: 0,
    });
    assert_eq!(
        invalid_symbol.write(),
        Err(ObjectError::InvalidReference {
            kind: "symbol",
            index: 9,
        })
    );

    let mut invalid_offset = object();
    invalid_offset.relocations.push(Relocation {
        section: SectionId(1),
        offset: 8,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: 0,
    });
    assert_eq!(
        invalid_offset.write(),
        Err(ObjectError::OutOfBounds {
            section: "relocation",
            offset: 8,
            size: 1,
        })
    );
}

#[test]
fn elf_relocation_kind_validation_is_target_specific() {
    let mut x86 = object();
    x86.relocations.push(Relocation {
        section: SectionId(1),
        offset: 0,
        kind: RelocKind::Branch26,
        symbol: SymbolRef::Local(0),
        addend: 0,
    });
    assert_eq!(
        x86.write(),
        Err(ObjectError::UnsupportedRelocation(RelocKind::Branch26))
    );

    let mut arm = object();
    arm.architecture = ElfArchitecture::Aarch64;
    arm.relocations = [
        RelocKind::Abs64,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
        RelocKind::Branch26,
        RelocKind::Adrp21,
        RelocKind::Add12,
        RelocKind::CondBranch19,
    ]
    .into_iter()
    .map(|kind| Relocation {
        section: SectionId(1),
        offset: 0,
        kind,
        symbol: SymbolRef::External("loader".into()),
        addend: 1,
    })
    .collect();
    let bytes = arm.write().expect("AArch64 relocation set");
    assert_eq!(
        ElfReader::validate(&bytes, ElfArchitecture::Aarch64),
        Ok(())
    );
}

#[test]
fn elf_validator_checks_architecture_and_reader_alias() {
    let bytes = object().write().expect("valid ELF");
    assert_eq!(ElfReader::validate(&bytes, ElfArchitecture::X86_64), Ok(()));
    assert_eq!(
        validate_elf(&bytes, ElfArchitecture::Aarch64),
        Err(ObjectError::InvalidField {
            field: "ELF machine",
            value: 62,
        })
    );
    assert_eq!(
        validate_elf(&bytes[..40], ElfArchitecture::X86_64),
        Err(ObjectError::Truncated {
            offset: 40,
            needed: 64,
        })
    );

    let mut table = bytes.clone();
    table[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_elf(&table, ElfArchitecture::X86_64),
        Err(ObjectError::OutOfBounds {
            section: "ELF section table",
            ..
        })
    ));
}

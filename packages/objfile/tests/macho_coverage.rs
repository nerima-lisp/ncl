//! Coverage tests for Mach-O writer paths.

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

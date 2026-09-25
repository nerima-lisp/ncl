//! Additional coverage tests for native object writers.

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

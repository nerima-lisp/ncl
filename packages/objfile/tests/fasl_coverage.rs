//! FASL wire-format validation coverage.
#![allow(clippy::expect_used)]

use ncl_objfile::*;

fn fasl() -> Fasl {
    Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 7,
        },
        sections: FaslSection {
            code: vec![0xc3, 0x90],
            relocations: vec![],
            constants: vec![1],
            symbols: vec![2],
            stack_maps: vec![3],
            debug: vec![4],
        },
    }
}

fn word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn fasl_header_rejects_each_incompatible_value() {
    let valid = FaslWriter::write(&fasl()).expect("valid FASL");
    let cases = [
        (8, 2u16.to_le_bytes().to_vec(), "version"),
        (10, vec![2], "architecture"),
        (11, vec![4], "pointer width"),
        (12, vec![2], "header"),
        (13, vec![32], "header"),
        (14, 1u16.to_le_bytes().to_vec(), "reserved"),
        (16, 8u64.to_le_bytes().to_vec(), "feature bitmap"),
    ];
    for (offset, replacement, field) in cases {
        let mut bytes = valid.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let error = FaslReader::read(&bytes, Architecture::X86_64, 7).expect_err(field);
        assert!(
            matches!(error, ObjectError::InvalidField { field: actual, .. } if actual == field)
        );
    }
    let error = FaslReader::read(&valid[..63], Architecture::X86_64, 7).expect_err("truncated");
    assert_eq!(
        error,
        ObjectError::Truncated {
            offset: 63,
            needed: 64
        }
    );
}

#[test]
fn fasl_reader_rejects_range_overflow_and_section_order() {
    let valid = FaslWriter::write(&fasl()).expect("valid FASL");
    let mut overflow = valid.clone();
    word(&mut overflow, 24, u32::MAX);
    word(&mut overflow, 28, 1);
    assert_eq!(
        FaslReader::read(&overflow, Architecture::X86_64, 7),
        Err(ObjectError::OutOfBounds {
            section: "code",
            offset: u64::from(u32::MAX),
            size: 1,
        })
    );

    let mut overlap = valid;
    word(&mut overlap, 32, 65);
    word(&mut overlap, 36, 0);
    assert_eq!(
        FaslReader::read(&overlap, Architecture::X86_64, 7),
        Err(ObjectError::Overlap {
            first: "previous FASL section",
            second: "relocation",
        })
    );
}

#[test]
fn fasl_relocation_round_trip_covers_all_wire_kinds() {
    let kinds = [
        RelocKind::Abs64,
        RelocKind::PcRel32,
        RelocKind::Plt32,
        RelocKind::Branch26,
        RelocKind::Adrp21,
        RelocKind::Add12,
        RelocKind::CondBranch19,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
    ];
    let mut value = fasl();
    value.sections.relocations = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Relocation {
            section: SectionId(0),
            offset: u32::try_from(index % 2).expect("small offset"),
            kind,
            symbol: SymbolRef::Local(0),
            addend: -4,
        })
        .collect();
    let bytes = FaslWriter::write(&value).expect("all relocation kinds fit");
    let decoded = FaslReader::read(&bytes, Architecture::X86_64, 7).expect("round trip");
    assert_eq!(decoded.sections.relocations, value.sections.relocations);
}

#[test]
fn fasl_rejects_bad_relocation_references_and_addends() {
    let mut value = fasl();
    value.sections.relocations = vec![Relocation {
        section: SectionId(1),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: 0,
    }];
    let bytes = FaslWriter::write(&value).expect("wire relocation");
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 7),
        Err(ObjectError::InvalidReference {
            kind: "FASL relocation section",
            index: 1,
        })
    );

    value.sections.relocations[0].section = SectionId(0);
    value.sections.relocations[0].offset = 2;
    let bytes = FaslWriter::write(&value).expect("wire relocation");
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 7),
        Err(ObjectError::OutOfBounds {
            section: "FASL relocation",
            offset: 2,
            size: 1,
        })
    );

    value.sections.relocations[0].offset = 0;
    value.sections.relocations[0].addend = i64::from(i32::MAX) + 1;
    assert_eq!(
        FaslWriter::write(&value),
        Err(ObjectError::InvalidField {
            field: "relocation addend",
            value: u64::try_from(i64::from(i32::MAX) + 1).expect("positive addend"),
        })
    );
}

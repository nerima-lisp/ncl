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
            code: vec![0xc3, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
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
        (8, 3u16.to_le_bytes().to_vec(), "version"),
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
            offset: if matches!(
                kind,
                RelocKind::Abs64 | RelocKind::CodeEntry | RelocKind::ExternalSymbol
            ) {
                0
            } else {
                u32::try_from(index % 2).expect("small offset")
            },
            kind,
            symbol: if index % 2 == 0 {
                SymbolRef::Local(u32::try_from(index + 1).expect("small index"))
            } else {
                SymbolRef::External(format!("external_{index}"))
            },
            addend: -4,
        })
        .collect();
    let bytes = FaslWriter::write(&value).expect("all relocation kinds fit");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 2);
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
    value.sections.relocations[0].offset = 1;
    let bytes = FaslWriter::write(&value).expect("wire relocation");
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 7),
        Err(ObjectError::OutOfBounds {
            section: "FASL relocation",
            offset: 1,
            size: 8,
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

#[test]
fn fasl_v1_reader_preserves_legacy_local_zero_relocations() {
    let mut bytes = vec![0; 88];
    bytes[0..8].copy_from_slice(b"NCLFASL\0");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[10] = Architecture::X86_64 as u8;
    bytes[11] = 8;
    bytes[12] = 1;
    bytes[13] = 64;
    bytes[16..24].copy_from_slice(&7u64.to_le_bytes());
    bytes[24..28].copy_from_slice(&64u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&8u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&72u32.to_le_bytes());
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    for offset in [40, 48, 56] {
        bytes[offset..offset + 4].copy_from_slice(&88u32.to_le_bytes());
    }
    bytes[72..76].copy_from_slice(&0u32.to_le_bytes());
    bytes[76..80].copy_from_slice(&0u32.to_le_bytes());
    bytes[80..84].copy_from_slice(&0u32.to_le_bytes());
    bytes[84..88].copy_from_slice(&(-3i32).to_le_bytes());

    let decoded = FaslReader::read(&bytes, Architecture::X86_64, 7).expect("legacy v1 FASL");
    assert_eq!(decoded.sections.relocations[0].symbol, SymbolRef::Local(0));
    assert_eq!(decoded.sections.relocations[0].addend, -3);
}

#[test]
fn fasl_reader_checks_full_relocation_widths() {
    let payload = fasl();
    let kinds = [
        (RelocKind::Abs64, 8usize),
        (RelocKind::PcRel32, 4),
        (RelocKind::Plt32, 4),
        (RelocKind::Branch26, 4),
        (RelocKind::Adrp21, 4),
        (RelocKind::Add12, 4),
        (RelocKind::CondBranch19, 4),
        (RelocKind::CodeEntry, 8),
        (RelocKind::ExternalSymbol, 8),
    ];
    for architecture in [Architecture::X86_64, Architecture::Aarch64] {
        for (kind, width) in kinds {
            let offset =
                u32::try_from(payload.sections.code.len() - width).expect("small exact-fit offset");
            let mut value = payload.clone();
            value.header.architecture = architecture;
            value.sections.relocations = vec![Relocation {
                section: SectionId(0),
                offset,
                kind,
                symbol: if kind == RelocKind::ExternalSymbol {
                    SymbolRef::External("loader".into())
                } else {
                    SymbolRef::Local(1)
                },
                addend: 0,
            }];
            let bytes = FaslWriter::write(&value).expect("wire exact-fit FASL");
            let decoded = FaslReader::read(&bytes, architecture, 7)
                .expect("relocation ending at code boundary");
            assert_eq!(decoded.sections.relocations, value.sections.relocations);

            let overflow_offset = offset + 1;
            value.sections.relocations[0].offset = overflow_offset;
            let bytes = FaslWriter::write(&value).expect("wire overflowing FASL");
            assert_eq!(
                FaslReader::read(&bytes, architecture, 7),
                Err(ObjectError::OutOfBounds {
                    section: "FASL relocation",
                    offset: u64::from(overflow_offset),
                    size: u64::try_from(width).expect("small width"),
                })
            );
        }
    }
}

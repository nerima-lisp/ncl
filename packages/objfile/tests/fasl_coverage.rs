//! FASL wire-format validation coverage.
#![allow(clippy::expect_used)]

use ncl_objfile::*;

#[test]
fn fasl_writer_emits_the_declared_wire_offsets_and_sizes() -> Result<(), ObjectError> {
    use std::ops::Range;

    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::Aarch64,
            features: 0x0102_0304_0506_0708,
        },
        sections: FaslSection {
            code: vec![0x11, 0x22],
            relocations: vec![],
            constants: vec![0x33, 0x44],
            symbols: vec![0x55],
            stack_maps: vec![0x66],
            debug: vec![0x77, 0x88],
        },
    };

    let bytes = FaslWriter::write(&value)?;
    let read_u32 = |range: Range<usize>| {
        let raw = bytes
            .get(range)
            .ok_or(ObjectError::InvalidStructure("invalid test byte range"))?;
        Ok(u32::from_le_bytes(raw.try_into().map_err(|_| {
            ObjectError::InvalidStructure("invalid test byte range")
        })?))
    };
    let read_u64 = |range: Range<usize>| {
        let raw = bytes
            .get(range)
            .ok_or(ObjectError::InvalidStructure("invalid test byte range"))?;
        Ok(u64::from_le_bytes(raw.try_into().map_err(|_| {
            ObjectError::InvalidStructure("invalid test byte range")
        })?))
    };
    let check_bytes = |range: Range<usize>, expected: &[u8]| {
        let actual = bytes
            .get(range)
            .ok_or(ObjectError::InvalidStructure("invalid test byte range"))?;
        if actual != expected {
            return Err(ObjectError::InvalidStructure("unexpected test bytes"));
        }
        Ok(())
    };
    let check_value = |actual, expected| {
        if actual != expected {
            return Err(ObjectError::InvalidStructure("unexpected test value"));
        }
        Ok(())
    };

    check_bytes(0..8, b"NCLFASL\0")?;
    check_bytes(8..14, &[1, 0, 2, 8, 1, 64])?;
    check_value(read_u64(16..24)?, 0x0102_0304_0506_0708)?;
    check_value(u64::from(read_u32(24..28)?), 64)?;
    check_value(u64::from(read_u32(28..32)?), 2)?;
    check_value(u64::from(read_u32(32..36)?), 66)?;
    check_value(u64::from(read_u32(36..40)?), 0)?;
    check_value(u64::from(read_u32(40..44)?), 66)?;
    check_value(u64::from(read_u32(44..48)?), 2)?;
    check_value(u64::from(read_u32(48..52)?), 68)?;
    check_value(u64::from(read_u32(52..56)?), 1)?;
    check_value(u64::from(read_u32(56..60)?), 69)?;
    check_value(u64::from(read_u32(60..64)?), 1)?;
    check_bytes(
        64..bytes.len(),
        &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
    )?;
    Ok(())
}

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

    let mut bad_magic = valid;
    bad_magic[0] = 0;
    assert_eq!(
        FaslReader::read(&bad_magic, Architecture::X86_64, 7),
        Err(ObjectError::InvalidField {
            field: "magic",
            value: 0,
        })
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

#[test]
fn fasl_rejects_each_out_of_bounds_section_and_invalid_wire_kind() {
    let valid = FaslWriter::write(&fasl()).expect("valid FASL");
    for (section_name, section, offset, wire_size, reported_size) in [
        ("relocation", 32, 0x1000, 1, 16u32),
        ("constant", 40, 0x1000, 1, 1u32),
        ("symbol", 48, 0x1000, 1, 1u32),
        ("stack map", 56, 0x1000, 1, 1u32),
    ] {
        let mut bytes = valid.clone();
        word(&mut bytes, section, offset);
        word(&mut bytes, section + 4, wire_size);
        assert_eq!(
            FaslReader::read(&bytes, Architecture::X86_64, 7),
            Err(ObjectError::OutOfBounds {
                section: section_name,
                offset: u64::from(offset),
                size: u64::from(reported_size),
            })
        );
    }

    let mut value = fasl();
    value.sections.code = vec![0; 16];
    value.sections.relocations = vec![Relocation {
        section: SectionId(0),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: 0,
    }];
    let mut bad_kind = FaslWriter::write(&value).expect("relocation fits");
    let relocation_offset = u32::from_le_bytes(
        bad_kind[32..36]
            .try_into()
            .expect("relocation offset field"),
    ) as usize;
    bad_kind[relocation_offset + 8..relocation_offset + 12].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        FaslReader::read(&bad_kind, Architecture::X86_64, 7),
        Err(ObjectError::InvalidField {
            field: "relocation kind",
            value: 99,
        })
    );
}

use super::*;
use crate::{FaslRejectReason, RelocKind, SectionId, SymbolRef};

#[test]
fn fasl_rejection_reasons_map_to_stable_keywords() {
    let cases = [
        (FaslRejectReason::BadMagic, ":BAD-MAGIC"),
        (FaslRejectReason::UnsupportedVersion, ":UNSUPPORTED-VERSION"),
        (FaslRejectReason::TargetMismatch, ":TARGET-MISMATCH"),
        (
            FaslRejectReason::RuntimeAbiMismatch,
            ":RUNTIME-ABI-MISMATCH",
        ),
        (
            FaslRejectReason::CompilerIdentityMismatch,
            ":COMPILER-MISMATCH",
        ),
        (
            FaslRejectReason::PolicyFingerprintMismatch,
            ":POLICY-MISMATCH",
        ),
        (
            FaslRejectReason::FeatureFingerprintMismatch,
            ":FEATURE-MISMATCH",
        ),
        (FaslRejectReason::MalformedDirectory, ":INVALID-SECTION"),
        (FaslRejectReason::ResourceLimitExceeded, ":RESOURCE-LIMIT"),
        (FaslRejectReason::MalformedMetadata, ":INVALID-METADATA"),
        (FaslRejectReason::InvalidReference, ":INVALID-REFERENCE"),
        (FaslRejectReason::InvalidCodeObject, ":INVALID-CODE-OBJECT"),
        (FaslRejectReason::InvalidRelocation, ":INVALID-RELOCATION"),
        (FaslRejectReason::InvalidSafepointMap, ":INVALID-STACK-MAP"),
        (
            FaslRejectReason::InvalidDebugRecord,
            ":INVALID-DEBUG-RECORD",
        ),
        (
            FaslRejectReason::InvalidTopLevelRecord,
            ":INVALID-TOPLEVEL-RECORD",
        ),
    ];
    assert_eq!(cases.len(), 16);
    for (reason, keyword) in cases {
        assert_eq!(reason.keyword(), keyword);
    }
}

#[test]
fn malformed_private_fasl_ranges_are_rejected() {
    assert_eq!(
        decode_relocations(&[0; 15], 1, 1),
        Err(ObjectError::InvalidField {
            field: "relocation size",
            value: 15,
        })
    );
    assert_eq!(
        range(&[], "u32", u32::MAX, u32::MAX),
        Err(ObjectError::OutOfBounds {
            section: "u32",
            offset: u64::from(u32::MAX),
            size: u64::from(u32::MAX),
        })
    );
    assert_eq!(
        validate_section_order(&[("overflow", u32::MAX, 1)]),
        Err(ObjectError::OutOfBounds {
            section: "overflow",
            offset: u64::from(u32::MAX),
            size: 1,
        })
    );
}

#[test]
fn private_wire_helpers_cover_integer_boundaries() {
    assert_eq!(
        u16_at(&[1], 0),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 2
        })
    );
    assert_eq!(
        u32_at(&[1, 2], 0),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 4
        })
    );
    assert_eq!(
        u64_at(&[1, 2], 0),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 8
        })
    );
    assert_eq!(
        relocation_wire::number_kind(99),
        Err(ObjectError::InvalidField {
            field: "relocation kind",
            value: 99,
        })
    );
    assert_eq!(relocation_wire::kind_number(RelocKind::ExternalSymbol), 8);
}

#[test]
fn private_fasl_range_parser_reports_short_headers_and_overlaps() {
    for length in [24usize, 28, 32, 36, 40, 44, 48, 52, 56, 60] {
        assert!(matches!(
            read_fasl_ranges(&vec![0; length], 1),
            Err(ObjectError::Truncated { needed: 4, .. })
        ));
    }
    let mut bytes = vec![0; 64];
    bytes[24..28].copy_from_slice(&64u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        read_fasl_ranges(&bytes, 1),
        Err(ObjectError::OutOfBounds {
            section: "code",
            offset: 64,
            size: 1,
        })
    ));
    let mut huge_relocation_count = vec![0; 64];
    huge_relocation_count[32..36].copy_from_slice(&64u32.to_le_bytes());
    huge_relocation_count[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        read_fasl_ranges(&huge_relocation_count, 1),
        Err(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::MAX,
        })
    ));
    assert_eq!(
        validate_section_order(&[("first", 64, 2), ("second", 65, 0)]),
        Err(ObjectError::Overlap {
            first: "previous FASL section",
            second: "second",
        })
    );
}

#[test]
fn private_fasl_helpers_round_trip_payload_and_relocations() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::Aarch64,
            features: 3,
        },
        sections: FaslSection {
            code: vec![1, 2],
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 1,
                kind: RelocKind::Add12,
                symbol: SymbolRef::Local(0),
                addend: -2,
            }],
            constants: vec![3],
            symbols: vec![4],
            stack_maps: vec![5],
            debug: vec![6],
        },
    };
    let bytes_result = FaslWriter::write(&value);
    assert!(bytes_result.is_ok());
    let Ok(bytes) = bytes_result else { return };
    let ranges_result = read_fasl_ranges(&bytes, 2);
    assert!(ranges_result.is_ok());
    let Ok(ranges) = ranges_result else { return };
    assert_eq!(ranges.code, &[1, 2]);
    assert_eq!(ranges.constants, &[3]);
    assert_eq!(ranges.symbols, &[4]);
    assert_eq!(ranges.stack, &[5]);
    assert_eq!(
        &bytes[usize::try_from(ranges.debug_start).unwrap_or(0)..],
        &[6]
    );
    assert_eq!(
        decode_relocations(ranges.reloc, 2, 1).map_or(0, |relocations| relocations.len()),
        1
    );
    assert_eq!(
        validate_section_order(&[("code", 64, 2), ("next", 66, 1)]),
        Ok(())
    );
}

fn legacy_value(relocations: Vec<Relocation>) -> Fasl {
    Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![0; 8],
            relocations,
            constants: vec![1],
            symbols: vec![2],
            stack_maps: vec![3],
            debug: vec![4],
        },
    }
}

fn legacy_bytes(value: &Fasl) -> Vec<u8> {
    let result = FaslWriter::write(value);
    assert!(result.is_ok());
    let Ok(bytes) = result else { return Vec::new() };
    bytes
}

fn assert_legacy_header_field(bytes: &[u8], field: &'static str) {
    assert!(matches!(
        FaslReader::read(bytes, Architecture::X86_64, 0),
        Err(ObjectError::InvalidField { field: actual, .. }) if actual == field
    ));
}

#[test]
fn legacy_reader_accepts_both_versions_and_reports_header_failures() {
    let value = legacy_value(Vec::new());
    let v2 = legacy_bytes(&value);
    assert_eq!(
        FaslReader::read(&v2, Architecture::X86_64, 0),
        Ok(value.clone())
    );

    let mut v1 = v2.clone();
    v1[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(FaslReader::read(&v1, Architecture::X86_64, 0), Ok(value));
    assert!(matches!(
        FaslReader::read(&v2[..63], Architecture::X86_64, 0),
        Err(ObjectError::Truncated { needed: 64, .. })
    ));

    for (offset, byte, field) in [
        (0, b'X', "magic"),
        (8, 3, "version"),
        (10, Architecture::Aarch64.wire_tag(), "architecture"),
        (11, 4, "pointer width"),
        (12, 0, "header"),
        (14, 1, "reserved"),
        (16, 1, "feature bitmap"),
    ] {
        let mut malformed = v2.clone();
        malformed[offset] = byte;
        assert_legacy_header_field(&malformed, field);
    }
    let mut bad_header_size = v2;
    bad_header_size[12] = 1;
    bad_header_size[13] = 63;
    assert_legacy_header_field(&bad_header_size, "header");
}

#[test]
fn legacy_reader_rejects_invalid_relocation_references_and_sites() {
    let relocation = |section, offset| Relocation {
        section: SectionId(section),
        offset,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: 0,
    };
    let wrong_section = legacy_bytes(&legacy_value(vec![relocation(1, 0)]));
    assert!(matches!(
        FaslReader::read(&wrong_section, Architecture::X86_64, 0),
        Err(ObjectError::InvalidReference {
            kind: "FASL relocation section",
            index: 1
        })
    ));

    let wrong_site = legacy_bytes(&legacy_value(vec![relocation(0, 1)]));
    assert!(matches!(
        FaslReader::read(&wrong_site, Architecture::X86_64, 0),
        Err(ObjectError::OutOfBounds {
            section: "FASL relocation",
            offset: 1,
            size: 8
        })
    ));
}

#[test]
fn legacy_writer_and_ranges_reject_unrepresentable_values() {
    let bad_addend = legacy_value(vec![Relocation {
        section: SectionId(0),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: i64::MAX,
    }]);
    assert!(matches!(
        FaslWriter::write(&bad_addend),
        Err(ObjectError::InvalidField {
            field: "relocation addend",
            ..
        })
    ));

    let mut underflow = legacy_bytes(&legacy_value(Vec::new()));
    underflow[40..44].copy_from_slice(&64_u32.to_le_bytes());
    assert!(matches!(
        read_fasl_ranges(&underflow, 2),
        Err(ObjectError::Overlap {
            first: "relocation",
            second: "constant"
        })
    ));

    let mut relocation_size_overflow = legacy_bytes(&legacy_value(Vec::new()));
    relocation_size_overflow[8..10].copy_from_slice(&1_u16.to_le_bytes());
    relocation_size_overflow[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        read_fasl_ranges(&relocation_size_overflow, 1),
        Err(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::MAX
        })
    ));

    assert!(matches!(
        fixed_array::<4>(&[], usize::MAX),
        Err(ObjectError::Truncated {
            offset: usize::MAX,
            needed: 4
        })
    ));
    assert!(matches!(
        write_at(&mut [0], usize::MAX, &[1], "test"),
        Err(ObjectError::OutOfBounds {
            section: "test",
            ..
        })
    ));
    assert!(matches!(
        write_at(&mut [0], 1, &[1], "test"),
        Err(ObjectError::OutOfBounds {
            section: "test",
            ..
        })
    ));
}

#[test]
fn legacy_writer_and_reader_accept_empty_sections_at_the_header_end() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: Vec::new(),
            relocations: Vec::new(),
            constants: Vec::new(),
            symbols: Vec::new(),
            stack_maps: Vec::new(),
            debug: Vec::new(),
        },
    };
    let bytes = legacy_bytes(&value);
    assert_eq!(bytes.len(), 64);
    for offset in [24, 32, 40, 48, 56] {
        assert_eq!(u32_at(&bytes, offset), Ok(64), "section offset at {offset}");
    }
    for offset in [28, 36, 44, 52, 60] {
        assert_eq!(u32_at(&bytes, offset), Ok(0), "section size at {offset}");
    }
    assert_eq!(FaslReader::read(&bytes, Architecture::X86_64, 0), Ok(value));
}

#[test]
fn legacy_v1_reader_decodes_a_fixed_core_relocation() {
    let value = legacy_value(vec![Relocation {
        section: SectionId(0),
        offset: 0,
        kind: RelocKind::Abs64,
        symbol: SymbolRef::Local(0),
        addend: -9,
    }]);
    let mut bytes = legacy_bytes(&value);
    assert_eq!(u32_at(&bytes, 32), Ok(72));
    assert_eq!(u32_at(&bytes, 40), Ok(96));

    assert_eq!(bytes.drain(88..96).count(), 8);
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[40..44].copy_from_slice(&88_u32.to_le_bytes());
    bytes[48..52].copy_from_slice(&89_u32.to_le_bytes());
    bytes[56..60].copy_from_slice(&90_u32.to_le_bytes());

    assert_eq!(FaslReader::read(&bytes, Architecture::X86_64, 0), Ok(value));
}

#[test]
fn legacy_v2_writer_and_reader_round_trip_external_relocations() {
    let value = legacy_value(vec![Relocation {
        section: SectionId(0),
        offset: 0,
        kind: RelocKind::ExternalSymbol,
        symbol: SymbolRef::External("native_entry".to_owned()),
        addend: -7,
    }]);
    let bytes = legacy_bytes(&value);
    assert_eq!(FaslReader::read(&bytes, Architecture::X86_64, 0), Ok(value));
}

#[test]
fn legacy_reader_rejects_invalid_external_relocation_payloads() {
    let value = legacy_value(vec![Relocation {
        section: SectionId(0),
        offset: 0,
        kind: RelocKind::ExternalSymbol,
        symbol: SymbolRef::External("x".to_owned()),
        addend: 0,
    }]);
    let valid = legacy_bytes(&value);

    let mut bad_symbol_kind = valid.clone();
    bad_symbol_kind[88..92].copy_from_slice(&9_u32.to_le_bytes());
    assert!(matches!(
        FaslReader::read(&bad_symbol_kind, Architecture::X86_64, 0),
        Err(ObjectError::InvalidField {
            field: "relocation symbol kind",
            value: 9,
        })
    ));

    let mut invalid_utf8 = valid.clone();
    invalid_utf8[96] = 0xff;
    assert_eq!(
        FaslReader::read(&invalid_utf8, Architecture::X86_64, 0),
        Err(ObjectError::InvalidName)
    );

    let mut truncated_name = valid;
    truncated_name[92..96].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        FaslReader::read(&truncated_name, Architecture::X86_64, 0),
        Err(ObjectError::Truncated {
            offset: 24,
            needed: 2,
        })
    );
}

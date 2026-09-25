use super::*;

#[test]
fn malformed_private_fasl_ranges_are_rejected() {
    assert_eq!(
        decode_relocations(&[0; 15]),
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
        number_kind(99),
        Err(ObjectError::InvalidField {
            field: "relocation kind",
            value: 99,
        })
    );
    assert_eq!(kind_number(RelocKind::ExternalSymbol), 8);
}

#[test]
fn private_fasl_range_parser_reports_short_headers_and_overlaps() {
    for length in [24usize, 28, 32, 36, 40, 44, 48, 52, 56, 60] {
        assert!(matches!(
            read_fasl_ranges(&vec![0; length]),
            Err(ObjectError::Truncated { needed: 4, .. })
        ));
    }
    let mut bytes = vec![0; 64];
    bytes[24..28].copy_from_slice(&64u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        read_fasl_ranges(&bytes),
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
        read_fasl_ranges(&huge_relocation_count),
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

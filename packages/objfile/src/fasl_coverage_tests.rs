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

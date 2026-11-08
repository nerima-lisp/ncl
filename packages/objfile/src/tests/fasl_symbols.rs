use super::*;

fn must_succeed<T, E>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|_| panic!("test fixture must be valid"))
}

fn symbol(id: u32, binding_kind: SymbolBindingKind, package: &str, name: &str) -> FaslSymbol {
    FaslSymbol {
        id,
        binding_kind,
        package: package.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn symbol_section_size_boundary_preserves_encoder_and_decoder_errors() {
    assert_eq!(check_section_size(MAX_SECTION_BYTES, None), Ok(()));
    assert_eq!(
        check_section_size(MAX_SECTION_BYTES + 1, None),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
    assert_eq!(check_section_size(MAX_SECTION_BYTES, Some(320)), Ok(()));
    assert_eq!(
        check_section_size(MAX_SECTION_BYTES + 1, Some(320)),
        Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
    );
}

#[test]
fn symbol_section_matches_canonical_bytes_and_round_trips() {
    let symbols = [FaslSymbol {
        id: 0,
        binding_kind: SymbolBindingKind::Lisp,
        package: "CL-USER".into(),
        name: "X".into(),
    }];
    let expected = [
        1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, b'C', b'L', b'-', b'U', b'S',
        b'E', b'R', 1, 0, 0, 0, b'X',
    ];
    let encoded = encode(&symbols);
    assert_eq!(encoded, Ok(expected.to_vec()));
    assert_eq!(decode(&expected, 320), Ok(symbols.to_vec()));

    assert_eq!(encode(&[]), Ok(vec![0, 0, 0, 0]));
    assert_eq!(decode(&[0, 0, 0, 0], 320), Ok(Vec::new()));
    let empty_text = [symbol(0, SymbolBindingKind::Lisp, "", "")];
    assert_eq!(
        decode(&must_succeed(encode(&empty_text)), 320),
        Ok(empty_text.to_vec())
    );
}

#[test]
fn symbol_section_rejects_bad_order_flags_strings_and_padding() {
    let valid = encode(&[FaslSymbol {
        id: 0,
        binding_kind: SymbolBindingKind::Lisp,
        package: String::new(),
        name: "A".into(),
    }]);
    let valid = must_succeed(valid);

    let mut bad_flags = valid.clone();
    bad_flags[12..16].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        decode(&bad_flags, 320).map(|_| ()),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 332))
    );

    let mut bad_padding = valid;
    let last = bad_padding.len() - 1;
    bad_padding[last] = 1;
    assert_eq!(
        decode(&bad_padding, 320).map(|_| ()),
        Err(FaslError::at(
            FaslRejectReason::InvalidReference,
            320 + last
        ))
    );

    let unsorted = [
        FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::NativeFunction,
            package: "P".into(),
            name: "Z".into(),
        },
        FaslSymbol {
            id: 1,
            binding_kind: SymbolBindingKind::Lisp,
            package: "P".into(),
            name: "A".into(),
        },
    ];
    assert_eq!(
        encode(&unsorted),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );
    assert_eq!(
        encode(&[FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::Lisp,
            package: "P\0".into(),
            name: "A".into(),
        }]),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );
    assert_eq!(
        encode(&[FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::Lisp,
            package: "P".into(),
            name: "A\0".into(),
        }]),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );
}

#[test]
fn symbol_encoder_orders_binding_kinds_and_rejects_duplicate_or_sparse_ids() {
    let symbols = [
        symbol(0, SymbolBindingKind::Lisp, "P", "A"),
        symbol(1, SymbolBindingKind::NativeFunction, "NATIVE", "f"),
    ];
    assert_eq!(
        decode(&must_succeed(encode(&symbols)), 320),
        Ok(symbols.to_vec())
    );

    let lexical = [
        symbol(0, SymbolBindingKind::Lisp, "P", "A"),
        symbol(1, SymbolBindingKind::Lisp, "P", "B"),
    ];
    assert_eq!(
        decode(&must_succeed(encode(&lexical)), 320),
        Ok(lexical.to_vec())
    );
    let descending = [lexical[1].clone(), lexical[0].clone()];
    assert_eq!(
        encode(&descending),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );

    let duplicate = [
        symbol(0, SymbolBindingKind::Lisp, "P", "A"),
        symbol(1, SymbolBindingKind::Lisp, "P", "A"),
    ];
    assert_eq!(
        encode(&duplicate),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );

    let sparse = [symbol(1, SymbolBindingKind::Lisp, "P", "A")];
    assert_eq!(
        encode(&sparse),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );

    let oversized = "x".repeat(MAX_STRING_BYTES + 1);
    assert_eq!(
        encode(&[symbol(0, SymbolBindingKind::Lisp, &oversized, "")]),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );

    let too_many = vec![symbol(0, SymbolBindingKind::Lisp, "", ""); MAX_RECORDS + 1];
    assert_eq!(
        encode(&too_many),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
}

#[test]
fn symbol_encoder_rejects_nul_in_package_and_name() {
    assert_eq!(
        encode(&[symbol(0, SymbolBindingKind::Lisp, "P\0", "A")]),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );
    assert_eq!(
        encode(&[symbol(0, SymbolBindingKind::Lisp, "P", "A\0")]),
        Err(FaslError::new(FaslRejectReason::InvalidReference))
    );
}

#[test]
fn symbol_decoder_rejects_unknown_tags_invalid_text_counts_and_trailing_bytes() {
    let valid = must_succeed(encode(&[symbol(0, SymbolBindingKind::Lisp, "P", "A")]));
    for prefix_end in 0..valid.len() {
        assert!(matches!(
            decode(&valid[..prefix_end], 320),
            Err(FaslError {
                reason: FaslRejectReason::InvalidReference,
                ..
            })
        ));
    }
    let mut unknown_kind = valid.clone();
    unknown_kind[8..12].copy_from_slice(&3_u32.to_le_bytes());
    assert_eq!(
        decode(&unknown_kind, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 328))
    );

    let mut bad_id = valid.clone();
    bad_id[4..8].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        decode(&bad_id, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 324))
    );

    let mut invalid_utf8 = valid.clone();
    invalid_utf8[25] = 0xff;
    assert_eq!(
        decode(&invalid_utf8, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 345))
    );

    let mut nul_text = valid.clone();
    nul_text[25] = 0;
    assert_eq!(
        decode(&nul_text, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 345))
    );

    let mut oversized_text = valid.clone();
    oversized_text[16..20].copy_from_slice(&0x0010_0001_u32.to_le_bytes());
    assert_eq!(
        decode(&oversized_text, 320),
        Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 336))
    );

    let mut truncated_text = valid.clone();
    truncated_text[16..20].copy_from_slice(&100_u32.to_le_bytes());
    assert_eq!(
        decode(&truncated_text, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 340))
    );

    assert_eq!(
        decode(&0x0010_0001_u32.to_le_bytes(), 320),
        Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
    );
    assert_eq!(
        decode(&[1, 0, 0, 0], 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 320))
    );

    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        decode(&trailing, 320),
        Err(FaslError::at(
            FaslRejectReason::InvalidReference,
            trailing.len() - 1 + 320
        ))
    );
}

#[test]
fn symbol_decoder_rejects_duplicate_names_with_dense_wire_ids() {
    let one = must_succeed(encode(&[symbol(0, SymbolBindingKind::Lisp, "P", "A")]));
    let mut duplicate = vec![2, 0, 0, 0];
    duplicate.extend_from_slice(&one[4..]);
    let mut second = one[4..].to_vec();
    second[..4].copy_from_slice(&1_u32.to_le_bytes());
    duplicate.extend_from_slice(&second);
    assert_eq!(
        decode(&duplicate, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 348))
    );

    let ordered = [
        symbol(0, SymbolBindingKind::Lisp, "P", "A"),
        symbol(1, SymbolBindingKind::Lisp, "P", "B"),
    ];
    let mut descending = must_succeed(encode(&ordered));
    descending[25] = b'B';
    descending[49] = b'A';
    assert_eq!(
        decode(&descending, 320),
        Err(FaslError::at(FaslRejectReason::InvalidReference, 348))
    );
}

#[test]
fn symbol_decoder_reaches_truncated_following_fixed_fields_after_text() {
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&24_u32.to_le_bytes());
    bytes.extend_from_slice(&[b'P'; 24]);
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(b'A');
    bytes.extend_from_slice(&[0; 3]);
    let mut second_header = Vec::new();
    second_header.extend_from_slice(&1_u32.to_le_bytes());
    second_header.extend_from_slice(&1_u32.to_le_bytes());
    second_header.extend_from_slice(&0_u32.to_le_bytes());

    for (prefix, field_offset) in [(3, 0), (7, 4), (11, 8)] {
        let mut truncated = bytes.clone();
        truncated.extend_from_slice(&second_header[..prefix]);
        assert_eq!(
            decode(&truncated, 320),
            Err(FaslError::at(
                FaslRejectReason::InvalidReference,
                372 + field_offset
            )),
            "second symbol prefix length {prefix}"
        );
    }
}

#[cfg(test)]
mod private_tests {
    use super::super::*;
    use crate::fasl_symbols::{FaslSymbol, SymbolBindingKind};
    use crate::fasl_validate::validate_references;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    fn group(constants: Vec<FaslConstant>) -> FaslConstantGroup {
        FaslConstantGroup {
            code_object_id: 0,
            constants,
        }
    }

    fn symbols() -> [FaslSymbol; 1] {
        [FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::Lisp,
            package: "CORE".to_owned(),
            name: "X".to_owned(),
        }]
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }

    fn raw_constant(tag: u16, flags: u16, payload: &[u8]) -> Vec<u8> {
        let mut writer = crate::fasl_records::SectionWriter::new();
        writer.put_u32(1);
        writer.put_u32(0);
        writer.put_u32(1);
        writer.put_u16(tag);
        writer.put_u16(flags);
        writer.put_u32(must_succeed(u32::try_from(payload.len())));
        writer.put_bytes(payload);
        writer.zero_padding(4);
        writer.into_bytes()
    }

    #[test]
    fn every_constant_tag_round_trips_with_canonical_little_endian_bytes() {
        let constants = vec![
            FaslConstant::Nil,
            FaslConstant::Truth,
            FaslConstant::Fixnum(FIXNUM_MIN),
            FaslConstant::Fixnum(FIXNUM_MAX),
            FaslConstant::Character('λ'),
            FaslConstant::SingleFloat(0x7fc0_0001),
            FaslConstant::DoubleFloat(0x7ff8_0000_0000_0001),
            FaslConstant::String("abc".to_owned()),
            FaslConstant::SymbolReference(0),
            FaslConstant::Cons { car: 0, cdr: 1 },
            FaslConstant::SimpleVector(vec![2, 3]),
            FaslConstant::Bignum {
                negative: true,
                limbs: vec![u32::MAX, 1],
            },
            FaslConstant::Ratio {
                numerator: 2,
                denominator: 3,
            },
            FaslConstant::Complex {
                real: 5,
                imaginary: 6,
            },
        ];
        let groups = [group(constants)];
        let bytes = must_succeed(encode(&groups));
        assert_eq!(decode(&bytes, 320), Ok(groups.to_vec()));

        let mut tags = Vec::new();
        let mut cursor = 12;
        for _ in 0..u32_at(&bytes, 8) {
            tags.push(u16_at(&bytes, cursor));
            let payload_length = must_succeed(usize::try_from(u32_at(&bytes, cursor + 4)));
            cursor = (cursor + 8 + payload_length + 3) & !3;
        }
        assert_eq!(tags, [0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(&bytes[36..44], &FIXNUM_MIN.to_le_bytes());
        assert_eq!(bytes[bytes.len() - 1], 0);
        must_succeed(validate_references(&groups, &symbols()));
    }

    #[test]
    fn constant_decoder_rejects_every_strict_prefix_for_each_tag() {
        let constants = [
            FaslConstant::Nil,
            FaslConstant::Truth,
            FaslConstant::Fixnum(0),
            FaslConstant::Character('x'),
            FaslConstant::SingleFloat(0x3f80_0000),
            FaslConstant::DoubleFloat(0x3ff0_0000_0000_0000),
            FaslConstant::String("x".to_owned()),
            FaslConstant::SymbolReference(0),
            FaslConstant::Cons { car: 0, cdr: 0 },
            FaslConstant::SimpleVector(vec![0]),
            FaslConstant::Bignum {
                negative: false,
                limbs: vec![1],
            },
            FaslConstant::Ratio {
                numerator: 0,
                denominator: 0,
            },
            FaslConstant::Complex {
                real: 0,
                imaginary: 0,
            },
        ];
        for (tag, constant) in constants.into_iter().enumerate() {
            let bytes = must_succeed(encode(&[group(vec![constant])]));
            for end in 0..bytes.len() {
                assert!(
                    matches!(
                        decode(&bytes[..end], 320),
                        Err(FaslError {
                            reason: FaslRejectReason::InvalidCodeObject,
                            ..
                        })
                    ),
                    "tag {tag} accepted strict prefix ending at {end}"
                );
            }
        }
    }

    #[test]
    fn constants_reject_bad_padding_lengths_fixnums_and_references() {
        let one_string = [group(vec![FaslConstant::String("x".to_owned())])];
        let mut bad_padding = must_succeed(encode(&one_string));
        bad_padding[21] = 1;
        assert_eq!(
            decode(&bad_padding, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 341))
        );

        let one_fixnum = [group(vec![FaslConstant::Fixnum(0)])];
        let mut bad_length = must_succeed(encode(&one_fixnum));
        bad_length[16..20].copy_from_slice(&7_u32.to_le_bytes());
        assert!(matches!(
            decode(&bad_length, 320),
            Err(FaslError {
                reason: FaslRejectReason::InvalidCodeObject,
                ..
            })
        ));

        assert_eq!(
            encode(&[group(vec![FaslConstant::Fixnum(i64::MIN)])]).map(|_| ()),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(
            encode(&[group(vec![FaslConstant::Fixnum(i64::MAX)])]).map(|_| ()),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );

        let bad_index = [group(vec![FaslConstant::Cons { car: 1, cdr: 0 }])];
        assert_eq!(
            validate_references(&bad_index, &symbols()),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
        let native_symbol = [FaslSymbol {
            binding_kind: SymbolBindingKind::NativeFunction,
            ..symbols()[0].clone()
        }];
        let symbol_reference = [group(vec![FaslConstant::SymbolReference(0)])];
        assert_eq!(
            validate_references(&symbol_reference, &native_symbol),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
    }

    #[test]
    fn constants_reject_unknown_tags_flags_and_unconsumed_payloads() {
        assert_eq!(
            decode(&raw_constant(13, 0, &[]), 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 332))
        );
        assert_eq!(
            decode(&raw_constant(0, 1, &[]), 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 334))
        );
        assert_eq!(
            decode(&raw_constant(0, 0, &[0]), 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
        );

        let mut bad_character = must_succeed(encode(&[group(vec![FaslConstant::Character('A')])]));
        bad_character[20..24].copy_from_slice(&0xd800_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_character, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
        );

        let mut bad_string = must_succeed(encode(&[group(vec![FaslConstant::String("a".into())])]));
        bad_string[20] = 0xff;
        assert_eq!(
            decode(&bad_string, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
        );

        let oversized_string = vec![b'x'; MAX_STRING_BYTES + 1];
        let oversized_payload = raw_constant(6, 0, &oversized_string);
        assert_eq!(
            decode(&oversized_payload, 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 336))
        );

        let mut oversized_vector =
            must_succeed(encode(&[group(vec![FaslConstant::SimpleVector(vec![0])])]));
        oversized_vector[20..24].copy_from_slice(&0x0010_0001_u32.to_le_bytes());
        assert_eq!(
            decode(&oversized_vector, 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 340))
        );

        let mut truncated_vector =
            must_succeed(encode(&[group(vec![FaslConstant::SimpleVector(vec![0])])]));
        truncated_vector[20..24].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode(&truncated_vector, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
        );
    }

    #[test]
    fn constant_decoder_rejects_fixnums_outside_the_target_range() {
        for value in [i64::MIN, i64::MAX] {
            let mut bytes = must_succeed(encode(&[group(vec![FaslConstant::Fixnum(0)])]));
            bytes[20..28].copy_from_slice(&value.to_le_bytes());
            assert_eq!(
                decode(&bytes, 320),
                Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
            );
        }
    }

    #[test]
    fn bignums_enforce_sign_reserved_bytes_and_normalized_magnitudes() {
        let positive = [group(vec![FaslConstant::Bignum {
            negative: false,
            limbs: vec![1],
        }])];
        let bytes = must_succeed(encode(&positive));
        assert_eq!(decode(&bytes, 320), Ok(positive.to_vec()));

        let mut bad_sign = bytes.clone();
        bad_sign[20] = 2;
        assert_eq!(
            decode(&bad_sign, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 340))
        );

        let mut bad_reserved = bytes.clone();
        bad_reserved[21] = 1;
        assert_eq!(
            decode(&bad_reserved, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 341))
        );

        let mut zero_limb_count = bytes.clone();
        zero_limb_count[24..28].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            decode(&zero_limb_count, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 344))
        );

        let mut oversized_limb_count = bytes.clone();
        oversized_limb_count[24..28].copy_from_slice(&0x0010_0001_u32.to_le_bytes());
        assert_eq!(
            decode(&oversized_limb_count, 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 344))
        );

        let mut truncated_limbs = bytes.clone();
        truncated_limbs[24..28].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode(&truncated_limbs, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 344))
        );

        let mut leading_zero = bytes;
        leading_zero[28..32].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            decode(&leading_zero, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 344))
        );

        assert_eq!(
            encode(&[group(vec![FaslConstant::Bignum {
                negative: true,
                limbs: Vec::new(),
            }])])
            .map(|_| ()),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(
            encode(&[group(vec![FaslConstant::Bignum {
                negative: false,
                limbs: vec![1, 0],
            }])])
            .map(|_| ()),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
    }

    #[test]
    fn constants_reject_non_dense_groups_oversized_strings_and_vector_references() {
        assert_eq!(
            encode(&[FaslConstantGroup {
                code_object_id: 1,
                constants: Vec::new(),
            }])
            .map(|_| ()),
            Err(FaslError::new(FaslRejectReason::InvalidCodeObject))
        );
        assert_eq!(
            encode(&[group(vec![FaslConstant::String(
                "x".repeat(MAX_STRING_BYTES + 1),
            )])])
            .map(|_| ()),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );

        let bad_vector = [group(vec![FaslConstant::SimpleVector(vec![0, 1])])];
        assert_eq!(
            validate_references(&bad_vector, &symbols()),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
        let bad_symbol_id = [group(vec![FaslConstant::SymbolReference(1)])];
        assert_eq!(
            validate_references(&bad_symbol_id, &symbols()),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
    }

    #[test]
    fn constant_section_checks_dense_groups_and_bounded_counts() {
        let empty = [0, 0, 0, 0];
        assert_eq!(decode(&empty, 320), Ok(Vec::new()));
        assert_eq!(encode(&[]), Ok(empty.to_vec()));
        assert_eq!(
            decode(&[0, 0, 0, 0, 0], 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 324))
        );
        assert_eq!(
            decode(&0x0010_0001_u32.to_le_bytes(), 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 320))
        );
        assert_eq!(
            decode(&[1, 0, 0, 0], 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 320))
        );

        let mut bad_group_id = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        bad_group_id[4..8].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_group_id, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 324))
        );

        let too_many_constants = [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 16, 0];
        assert_eq!(
            decode(&too_many_constants, 320),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 328))
        );
        let missing_constant = [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        assert_eq!(
            decode(&missing_constant, 320),
            Err(FaslError::at(FaslRejectReason::InvalidCodeObject, 328))
        );
    }

    #[test]
    fn constant_section_size_limit_includes_the_exact_boundary() {
        assert!(!section_size_exceeds_limit(MAX_SECTION_BYTES));
        assert!(section_size_exceeds_limit(MAX_SECTION_BYTES + 1));
        assert_eq!(
            checked_section_growth(MAX_SECTION_BYTES, 0),
            Ok(MAX_SECTION_BYTES)
        );
        assert_eq!(
            checked_section_growth(MAX_SECTION_BYTES, 1),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );
    }

    #[test]
    fn constant_decoder_rejects_total_constants_above_the_limit() {
        let first_count = MAX_RECORDS / 2;
        let second_count = first_count + 1;
        let mut writer = crate::fasl_records::SectionWriter::new();
        writer.put_u32(2);
        writer.put_u32(0);
        writer.put_u32(must_succeed(u32::try_from(first_count)));
        for _ in 0..first_count {
            writer.put_u16(0);
            writer.put_u16(0);
            writer.put_u32(0);
        }
        writer.put_u32(1);
        let second_count_offset = 4 + 8 + first_count * 8 + 4;
        writer.put_u32(must_succeed(u32::try_from(second_count)));
        for _ in 0..second_count {
            writer.put_u16(0);
            writer.put_u16(0);
            writer.put_u32(0);
        }
        assert_eq!(
            decode(&writer.into_bytes(), 320),
            Err(FaslError::at(
                FaslRejectReason::ResourceLimitExceeded,
                320 + second_count_offset
            ))
        );
    }

    #[test]
    fn constant_encoder_rejects_record_and_payload_counts_above_the_limit() {
        {
            let groups = vec![group(Vec::new()); MAX_RECORDS + 1];
            assert_eq!(
                encode(&groups),
                Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
            );
        }
        {
            let constants = vec![FaslConstant::Nil; MAX_RECORDS + 1];
            assert_eq!(
                encode(&[group(constants)]),
                Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
            );
        }
        {
            let indices = vec![0; MAX_RECORDS + 1];
            assert_eq!(
                encode(&[group(vec![FaslConstant::SimpleVector(indices)])]),
                Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
            );
        }
        {
            let limbs = vec![1; MAX_RECORDS + 1];
            assert_eq!(
                encode(&[group(vec![FaslConstant::Bignum {
                    negative: false,
                    limbs,
                }])]),
                Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
            );
        }
        {
            let half = MAX_RECORDS / 2;
            let groups = [
                group(vec![FaslConstant::Nil; half + 1]),
                FaslConstantGroup {
                    code_object_id: 1,
                    constants: vec![FaslConstant::Nil; half],
                },
            ];
            assert_eq!(
                encode(&groups),
                Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
            );
        }
    }
}

#[cfg(test)]
#[path = "private_tests/fasl_constant_payload_tests.rs"]
mod payload_tests;

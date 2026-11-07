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
}

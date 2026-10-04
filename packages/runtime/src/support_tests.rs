#[cfg(test)]
mod included_tests {
    #![allow(clippy::panic)]

    use super::super::support::{encode_maps, resolve_constant};
    use crate::Runtime;
    use ncl_ir::{Constant, ConstantIndex, StructureKind};
    use ncl_object::{
        Word, car, cdr, complex_imag, complex_real, double_value, ratio_denominator,
        ratio_numerator, simple_vector_ref, string_length, string_ref,
    };

    #[test]
    #[allow(clippy::too_many_lines)]
    fn resolves_each_constant_shape_to_the_expected_value() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        let resolve = |constant: &Constant, previous: &[Word], runtime: &mut Runtime| {
            resolve_constant(&mut runtime.context, &runtime.object, constant, previous)
                .unwrap_or_else(|error| panic!("constant resolution failed: {error}"))
        };
        assert_eq!(
            resolve(&Constant::Fixnum(42), &[], &mut runtime),
            Word::fixnum(42)
        );
        assert_eq!(
            resolve(&Constant::Character(u32::from('A')), &[], &mut runtime),
            Word::character(u32::from('A'))
        );
        assert_eq!(resolve(&Constant::Nil, &[], &mut runtime), Word::NIL);
        assert_eq!(resolve(&Constant::T, &[], &mut runtime), Word::TRUE);
        assert_eq!(
            resolve(&Constant::Unbound, &[], &mut runtime),
            Word::UNBOUND
        );
        assert_eq!(
            resolve(
                &Constant::FunctionEntry(ncl_ir::FunctionId(7)),
                &[],
                &mut runtime
            ),
            Word::NIL
        );
        let string = resolve(&Constant::StringBytes(b"abc".to_vec()), &[], &mut runtime);
        assert_eq!(string_length(&runtime.context, string), Ok(3));
        assert_eq!(string_ref(&runtime.context, string, 1), Ok('b'));
        let previous = [Word::fixnum(7), Word::fixnum(8)];
        assert_eq!(
            resolve(&Constant::Object(ConstantIndex(1)), &previous, &mut runtime),
            Word::fixnum(8)
        );
        let cons = resolve(
            &Constant::Structure {
                kind: StructureKind::Cons,
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            },
            &previous,
            &mut runtime,
        );
        assert_eq!(car(&runtime.context, cons), Ok(Word::fixnum(7)));
        assert_eq!(cdr(&runtime.context, cons), Ok(Word::fixnum(8)));
        let vector = resolve(
            &Constant::Structure {
                kind: StructureKind::SimpleVector,
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            },
            &previous,
            &mut runtime,
        );
        assert_eq!(
            simple_vector_ref(&runtime.context, vector, 0),
            Ok(Word::fixnum(7))
        );
        assert_eq!(
            simple_vector_ref(&runtime.context, vector, 1),
            Ok(Word::fixnum(8))
        );
        let double = resolve(&Constant::DoubleFloat(1.5), &[], &mut runtime);
        assert_eq!(
            double_value(&runtime.context, ncl_object::DoubleFloat::from_word(double))
                .unwrap_or_else(|error| panic!("double constant: {error}"))
                .to_bits(),
            1.5_f64.to_bits()
        );
        let single = resolve(&Constant::SingleFloat(2.5), &[], &mut runtime);
        assert_eq!(
            double_value(&runtime.context, ncl_object::DoubleFloat::from_word(single))
                .unwrap_or_else(|error| panic!("single constant: {error}"))
                .to_bits(),
            2.5_f64.to_bits()
        );
        let symbol = resolve(
            &Constant::Symbol {
                package: "KEYWORD".to_owned(),
                name: "ANSWER".to_owned(),
            },
            &[],
            &mut runtime,
        );
        assert_eq!(runtime.format_result(symbol), ":ANSWER");
        let bignum = resolve(
            &Constant::Bignum {
                negative: true,
                limbs: vec![1, 2],
            },
            &[],
            &mut runtime,
        );
        assert_eq!(runtime.format_result(bignum), "-8589934593");
        let ratio = resolve(
            &Constant::Ratio {
                numerator: ConstantIndex(0),
                denominator: ConstantIndex(1),
            },
            &previous,
            &mut runtime,
        );
        let ratio = ncl_object::Ratio::from_word(ratio);
        assert_eq!(
            ratio_numerator(&runtime.context, ratio),
            Ok(Word::fixnum(7))
        );
        assert_eq!(
            ratio_denominator(&runtime.context, ratio),
            Ok(Word::fixnum(8))
        );
        let complex = resolve(
            &Constant::Complex {
                real: ConstantIndex(0),
                imaginary: ConstantIndex(1),
            },
            &previous,
            &mut runtime,
        );
        let complex = ncl_object::Complex::from_word(complex);
        assert_eq!(complex_real(&runtime.context, complex), Ok(Word::fixnum(7)));
        assert_eq!(complex_imag(&runtime.context, complex), Ok(Word::fixnum(8)));
    }

    #[test]
    fn rejects_invalid_constant_references_and_structure_shapes() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
        for constant in [
            Constant::Object(ConstantIndex(1)),
            Constant::Structure {
                kind: StructureKind::Cons,
                elements: vec![ConstantIndex(0)],
            },
        ] {
            assert!(
                resolve_constant(&mut runtime.context, &runtime.object, &constant, &[]).is_err()
            );
        }
        let invalid_utf8 = Constant::StringBytes(vec![0xff]);
        assert!(
            resolve_constant(&mut runtime.context, &runtime.object, &invalid_utf8, &[]).is_err()
        );
    }

    #[test]
    fn encodes_safepoint_maps_to_nonempty_bytes() {
        let map = ncl_codegen::SafepointMap::new(3, 4, 1, &[], &[], 0)
            .unwrap_or_else(|error| panic!("safepoint map: {error}"));
        let bytes = encode_maps(&[map]).unwrap_or_else(|error| panic!("map encoding: {error}"));
        assert!(!bytes.is_empty());
    }
}

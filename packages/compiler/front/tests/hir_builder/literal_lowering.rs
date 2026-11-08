use ncl_compiler_front::{
    Expr, Literal as FrontLiteral, NumberLiteral as FrontNumber, SymbolRef, TypeSpecifier,
};
use ncl_hir::{Literal as HirLiteral, LiteralElementType, NodeKind};
use ncl_ir::{
    Constant, ConstantIndex, Function, OpKind, SpecializedArrayElementType, StructureKind,
};
use ncl_object::ArrayElementType;

use super::common::{TestResult, build, context, lower_both_and_verify};

fn constant_at(function: &Function, index: ConstantIndex) -> Option<&Constant> {
    function.constants.get(usize::try_from(index.0).ok()?)
}

fn emits_constant(function: &Function, index: ConstantIndex) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .any(|op| matches!(&op.kind, OpKind::Const { result } if *result == index))
}

fn emits_value(function: &Function, expected: &Constant) -> bool {
    function
        .constants
        .iter()
        .enumerate()
        .any(|(index, constant)| {
            constant == expected
                && u32::try_from(index)
                    .ok()
                    .is_some_and(|index| emits_constant(function, ConstantIndex(index)))
        })
}

fn assert_array_storage<'a>(
    function: &'a Function,
    identity: u32,
    expected_kind: StructureKind,
    expected_elements: usize,
) -> &'a [ConstantIndex] {
    let value = function
        .constants
        .iter()
        .find_map(|constant| match constant {
            Constant::LiteralObject {
                identity: actual,
                value,
            } if *actual == identity => Some(*value),
            _ => None,
        })
        .expect("array descriptor");
    let Some(Constant::Structure { kind, elements }) = constant_at(function, value) else {
        panic!("array descriptor is not a structure")
    };
    assert_eq!(kind, &expected_kind);
    assert_eq!(elements.len(), expected_elements);
    elements
}

#[test]
fn numeric_character_and_string_literals_lower_to_exact_mir_values() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(57);
    let ratio = FrontNumber::Ratio {
        numerator: Box::new(FrontNumber::Bignum {
            negative: true,
            limbs: vec![3, 1],
        }),
        denominator: Box::new(FrontNumber::Fixnum(11)),
    };
    let complex = FrontNumber::Complex {
        real: Box::new(FrontNumber::DoubleFloat(-0.0)),
        imaginary: Box::new(FrontNumber::SingleFloat(0.0)),
    };
    let expr = Expr::Progn(vec![
        Expr::Constant(FrontLiteral::Number(FrontNumber::Bignum {
            negative: false,
            limbs: vec![u32::MAX, 9],
        })),
        Expr::Constant(FrontLiteral::Number(ratio)),
        Expr::Constant(FrontLiteral::Number(complex)),
        Expr::Constant(FrontLiteral::Character(0x1f642)),
        Expr::Constant(FrontLiteral::String(Vec::new())),
        Expr::Constant(FrontLiteral::String(vec!['λ', '🍁'])),
    ]);
    let form = build(
        &mut builder,
        id,
        &expr,
        "numeric and text literal constants",
    )?;
    let (hir, baseline) = lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS 2.3.1 defines numeric token classes; MIR keeps recursive parts and IEEE bits.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_ca.htm
    // CLHS 2.4.8.1 and 2.4.5 define character and string literal syntax.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_dha.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_de.htm
    for function in [&hir.entry, &baseline.entry] {
        assert!(emits_value(
            function,
            &Constant::Bignum {
                negative: false,
                limbs: vec![u32::MAX, 9],
            }
        ));
        let (numerator, denominator) = function
            .constants
            .iter()
            .find_map(|constant| match constant {
                Constant::Ratio {
                    numerator,
                    denominator,
                } => Some((*numerator, *denominator)),
                _ => None,
            })
            .expect("MIR ratio constant");
        assert_eq!(
            constant_at(function, numerator),
            Some(&Constant::Bignum {
                negative: true,
                limbs: vec![3, 1],
            })
        );
        assert_eq!(
            constant_at(function, denominator),
            Some(&Constant::Fixnum(11))
        );

        let (real, imaginary) = function
            .constants
            .iter()
            .find_map(|constant| match constant {
                Constant::Complex { real, imaginary } => Some((*real, *imaginary)),
                _ => None,
            })
            .expect("MIR complex constant");
        assert!(matches!(
            constant_at(function, real),
            Some(Constant::DoubleFloat(value)) if value.to_bits() == (-0.0_f64).to_bits()
        ));
        assert!(matches!(
            constant_at(function, imaginary),
            Some(Constant::SingleFloat(value)) if value.to_bits() == 0.0_f32.to_bits()
        ));
        assert!(function.constants.contains(&Constant::Character(0x1f642)));
        assert!(
            function
                .constants
                .contains(&Constant::StringBytes(Vec::new()))
        );
        assert!(
            function
                .constants
                .contains(&Constant::StringBytes("λ🍁".as_bytes().to_vec()))
        );
    }
    Ok(())
}

#[test]
fn vectors_and_array_literals_lower_with_rank_element_type_and_identity() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(59);
    let element_types = [
        ArrayElementType::Bit,
        ArrayElementType::Character,
        ArrayElementType::BaseChar,
        ArrayElementType::Fixnum,
        ArrayElementType::Signed,
        ArrayElementType::Unsigned,
        ArrayElementType::SingleFloat,
        ArrayElementType::DoubleFloat,
    ];
    let mir_element_types = [
        SpecializedArrayElementType::Bit,
        SpecializedArrayElementType::Character,
        SpecializedArrayElementType::BaseChar,
        SpecializedArrayElementType::Fixnum,
        SpecializedArrayElementType::Signed,
        SpecializedArrayElementType::Unsigned,
        SpecializedArrayElementType::SingleFloat,
        SpecializedArrayElementType::DoubleFloat,
    ];
    assert_eq!(element_types.len(), mir_element_types.len());
    let mut expressions = vec![
        Expr::Constant(FrontLiteral::Cons(
            Box::new(FrontLiteral::fixnum(2)),
            Box::new(FrontLiteral::Nil),
        )),
        Expr::Constant(FrontLiteral::Vector(vec![
            FrontLiteral::fixnum(4),
            FrontLiteral::T,
        ])),
    ];
    for (identity, element_type) in element_types.iter().copied().enumerate() {
        expressions.push(Expr::Constant(FrontLiteral::SpecializedArray {
            identity: u32::try_from(identity + 60)?,
            element_type,
            elements: vec![FrontLiteral::fixnum(6)],
        }));
    }
    expressions.push(Expr::Constant(FrontLiteral::Array {
        identity: 71,
        dimensions: vec![2, 1],
        element_type: ArrayElementType::Unsigned,
        elements: vec![FrontLiteral::fixnum(8), FrontLiteral::fixnum(9)],
    }));
    let repeated_array = FrontLiteral::Array {
        identity: 72,
        dimensions: vec![1],
        element_type: ArrayElementType::Character,
        elements: vec![FrontLiteral::Character(u32::from('λ'))],
    };
    expressions.extend([repeated_array.clone(), repeated_array].map(Expr::Constant));
    let expr = Expr::Progn(expressions);
    let form = build(&mut builder, id, &expr, "vectors and array literal storage")?;
    let (hir, baseline) = lower_both_and_verify(&builder, &form, &expr)?;

    // CLHS defines vectors in 2.4.8.3 and general array syntax in 2.4.8.12.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_dhc.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_dhl.htm
    // Array element type, rank, and dimensions are part of the array type.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_array.htm
    for function in [&hir.entry, &baseline.entry] {
        assert!(function.constants.iter().any(|constant| matches!(constant,
            Constant::Structure { kind: StructureKind::Cons, elements } if elements.len() == 2
        )));
        assert!(function.constants.iter().any(|constant| matches!(constant,
            Constant::Structure { kind: StructureKind::SimpleVector, elements } if elements.len() == 2
        )));

        for (offset, expected_type) in mir_element_types.iter().enumerate() {
            assert_array_storage(
                function,
                u32::try_from(offset + 60)?,
                StructureKind::SpecializedArray {
                    element_type: *expected_type,
                },
                1,
            );
        }
        assert_array_storage(
            function,
            71,
            StructureKind::Array {
                dimensions: vec![2, 1],
                element_type: ncl_ir::ArrayElementType::Unsigned,
            },
            2,
        );
        assert_eq!(
            function
                .constants
                .iter()
                .filter(|constant| matches!(constant, Constant::LiteralObject { identity: 72, .. }))
                .count(),
            1
        );
    }
    Ok(())
}

#[test]
fn opaque_type_specifier_storage_retains_nested_array_literals() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(61);
    let source_array = FrontLiteral::SpecializedArray {
        identity: 74,
        element_type: ArrayElementType::T,
        elements: vec![FrontLiteral::Nil, FrontLiteral::T],
    };
    let source_specifier = FrontLiteral::Cons(
        Box::new(FrontLiteral::Symbol(SymbolRef::interned(
            "COMMON-LISP-USER",
            "CUSTOM-TYPE",
        ))),
        Box::new(FrontLiteral::Cons(
            Box::new(source_array),
            Box::new(FrontLiteral::Nil),
        )),
    );
    let expr = Expr::The {
        type_specifier: TypeSpecifier::new(source_specifier),
        value: Box::new(Expr::Constant(FrontLiteral::fixnum(17))),
    };
    let form = build(
        &mut builder,
        id,
        &expr,
        "opaque type specifier literal storage",
    )?;
    let source_id = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::TypeAssertion { expected_type, .. } => Some(*expected_type),
            _ => None,
        })
        .expect("type assertion source constant");
    let stored = form
        .type_spec_constant(source_id)
        .expect("stored type specifier");
    assert!(matches!(stored, HirLiteral::Cons(operator, arguments)
        if matches!(operator.as_ref(), HirLiteral::Symbol(id)
            if builder.symbols().get(*id).is_some_and(|symbol| symbol.name() == "CUSTOM-TYPE"))
        && matches!(arguments.as_ref(), HirLiteral::Cons(array, tail)
            if matches!(array.as_ref(), HirLiteral::SpecializedArray {
                identity: 74, element_type: LiteralElementType::T, elements,
            } if elements == &[HirLiteral::Nil, HirLiteral::True])
            && tail.as_ref() == &HirLiteral::Nil)));

    let (hir, _baseline) = lower_both_and_verify(&builder, &form, &expr)?;
    let elements = assert_array_storage(
        &hir.entry,
        74,
        StructureKind::Array {
            dimensions: vec![2],
            element_type: ncl_ir::ArrayElementType::T,
        },
        2,
    );
    let first = elements
        .first()
        .copied()
        .ok_or_else(|| std::io::Error::other("first array element is missing"))?;
    let second = elements
        .get(1)
        .copied()
        .ok_or_else(|| std::io::Error::other("second array element is missing"))?;
    assert_eq!(constant_at(&hir.entry, first), Some(&Constant::Nil));
    assert_eq!(constant_at(&hir.entry, second), Some(&Constant::T));
    Ok(())
}

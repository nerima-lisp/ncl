use ncl_compiler_front::{Expr, Literal as FrontLiteral, NumberLiteral as FrontNumber, SymbolRef};
use ncl_hir::{HirForm, Literal as HirLiteral, LiteralElementType, NodeKind, NumberLiteral};
use ncl_object::ArrayElementType;

use super::common::{TestResult, build, context};

fn literal_nodes(form: &HirForm) -> Vec<HirLiteral> {
    form.nodes()
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            NodeKind::Literal(literal) => Some(literal.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn numeric_and_atomic_literals_preserve_source_values() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(51);
    let symbol = SymbolRef::interned("COMMON-LISP-USER", "VALUE");
    let expr = Expr::Progn(vec![
        Expr::Constant(FrontLiteral::Nil),
        Expr::Constant(FrontLiteral::T),
        Expr::Constant(FrontLiteral::Symbol(SymbolRef::interned(
            "COMMON-LISP",
            "NIL",
        ))),
        Expr::Constant(FrontLiteral::Symbol(SymbolRef::interned(
            "COMMON-LISP",
            "T",
        ))),
        Expr::Constant(FrontLiteral::Symbol(symbol)),
        Expr::Constant(FrontLiteral::Number(FrontNumber::Fixnum(13))),
        Expr::Constant(FrontLiteral::Number(FrontNumber::Bignum {
            negative: true,
            limbs: vec![7, 2],
        })),
        Expr::Constant(FrontLiteral::Number(FrontNumber::Ratio {
            numerator: Box::new(FrontNumber::Fixnum(3)),
            denominator: Box::new(FrontNumber::Fixnum(8)),
        })),
        Expr::Constant(FrontLiteral::Number(FrontNumber::SingleFloat(1.25))),
        Expr::Constant(FrontLiteral::Number(FrontNumber::DoubleFloat(-2.5))),
        Expr::Constant(FrontLiteral::Number(FrontNumber::Complex {
            real: Box::new(FrontNumber::Fixnum(2)),
            imaginary: Box::new(FrontNumber::SingleFloat(0.5)),
        })),
        Expr::Constant(FrontLiteral::Character(u32::from('λ'))),
        Expr::Constant(FrontLiteral::String(vec!['n', 'c', 'l'])),
        Expr::Constant(FrontLiteral::Pathname(vec!['a', '/', 'b'])),
    ]);
    let form = build(&mut builder, id, &expr, "numeric and atomic literals")?;
    let actual = literal_nodes(&form);

    // CLHS 2.3.1 defines numeric token forms; retain exact numeric kinds and float bits.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_ca.htm
    assert_eq!(actual.len(), 14);
    assert_eq!(
        actual.iter().take(4).cloned().collect::<Vec<_>>(),
        vec![
            HirLiteral::Nil,
            HirLiteral::True,
            HirLiteral::Nil,
            HirLiteral::True,
        ]
    );
    let HirLiteral::Symbol(symbol_id) = actual
        .get(4)
        .ok_or_else(|| std::io::Error::other("user symbol literal is missing"))?
    else {
        return Err(std::io::Error::other("user symbol literal lost its identity").into());
    };
    let source_symbol = builder
        .symbols()
        .get(*symbol_id)
        .ok_or_else(|| std::io::Error::other("user symbol is absent from the HIR table"))?;
    assert_eq!(source_symbol.package(), Some("COMMON-LISP-USER"));
    assert_eq!(source_symbol.name(), "VALUE");
    assert_eq!(
        actual.iter().skip(5).cloned().collect::<Vec<_>>(),
        vec![
            HirLiteral::Number(NumberLiteral::Fixnum(13)),
            HirLiteral::Number(NumberLiteral::Bignum {
                negative: true,
                limbs: vec![7, 2],
            }),
            HirLiteral::Number(NumberLiteral::Ratio {
                numerator: Box::new(NumberLiteral::Fixnum(3)),
                denominator: Box::new(NumberLiteral::Fixnum(8)),
            }),
            HirLiteral::Number(NumberLiteral::SingleFloat(1.25_f32.to_bits())),
            HirLiteral::Number(NumberLiteral::DoubleFloat((-2.5_f64).to_bits())),
            HirLiteral::Number(NumberLiteral::Complex {
                real: Box::new(NumberLiteral::Fixnum(2)),
                imaginary: Box::new(NumberLiteral::SingleFloat(0.5_f32.to_bits())),
            }),
            HirLiteral::Character(u32::from('λ')),
            HirLiteral::String(vec!['n', 'c', 'l']),
            HirLiteral::Pathname(vec!['a', '/', 'b']),
        ]
    );
    Ok(())
}

#[test]
fn nested_and_array_literals_preserve_shape_and_storage_type() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(52);
    let mut expressions = vec![
        Expr::Constant(FrontLiteral::Cons(
            Box::new(FrontLiteral::fixnum(1)),
            Box::new(FrontLiteral::Nil),
        )),
        Expr::Constant(FrontLiteral::Vector(vec![
            FrontLiteral::Nil,
            FrontLiteral::T,
        ])),
    ];
    let element_types = [
        (ArrayElementType::T, LiteralElementType::T),
        (ArrayElementType::Bit, LiteralElementType::Bit),
        (ArrayElementType::Character, LiteralElementType::Character),
        (ArrayElementType::BaseChar, LiteralElementType::BaseChar),
        (ArrayElementType::Fixnum, LiteralElementType::Fixnum),
        (ArrayElementType::Signed, LiteralElementType::Signed),
        (ArrayElementType::Unsigned, LiteralElementType::Unsigned),
        (
            ArrayElementType::SingleFloat,
            LiteralElementType::SingleFloat,
        ),
        (
            ArrayElementType::DoubleFloat,
            LiteralElementType::DoubleFloat,
        ),
    ];
    for (identity, (source_type, _)) in element_types.iter().copied().enumerate() {
        expressions.push(Expr::Constant(FrontLiteral::SpecializedArray {
            identity: u32::try_from(identity)?,
            element_type: source_type,
            elements: vec![FrontLiteral::fixnum(6)],
        }));
    }
    expressions.push(Expr::Constant(FrontLiteral::Array {
        identity: 21,
        dimensions: vec![1, 2],
        element_type: ArrayElementType::T,
        elements: vec![FrontLiteral::fixnum(4), FrontLiteral::fixnum(9)],
    }));
    let expr = Expr::Progn(expressions);
    let form = build(&mut builder, id, &expr, "nested and array literals")?;
    let actual = literal_nodes(&form);

    // CLHS sharpsign syntax includes vectors, arrays, and pathnames; arrays retain their element type.
    // https://www.lispworks.com/documentation/HyperSpec/Body/02_dh.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/15_ab.htm
    assert_eq!(
        actual.first(),
        Some(&HirLiteral::Cons(
            Box::new(HirLiteral::Number(NumberLiteral::Fixnum(1))),
            Box::new(HirLiteral::Nil)
        ))
    );
    assert_eq!(
        actual.get(1),
        Some(&HirLiteral::Vector(vec![HirLiteral::Nil, HirLiteral::True]))
    );
    for (index, (_, expected_type)) in element_types.iter().enumerate() {
        assert_eq!(
            actual.get(index + 2),
            Some(&HirLiteral::SpecializedArray {
                identity: u32::try_from(index)?,
                element_type: *expected_type,
                elements: vec![HirLiteral::Number(NumberLiteral::Fixnum(6))],
            })
        );
    }
    assert_eq!(
        actual.last(),
        Some(&HirLiteral::Array {
            identity: 21,
            dimensions: vec![1, 2],
            element_type: LiteralElementType::T,
            elements: vec![
                HirLiteral::Number(NumberLiteral::Fixnum(4)),
                HirLiteral::Number(NumberLiteral::Fixnum(9)),
            ],
        })
    );
    Ok(())
}

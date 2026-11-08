use ncl_compiler_front::{Expr, Literal, NumberLiteral, SymbolRef, TypeSpecifier};
use ncl_hir::{BuiltinType, ClTypeKind, HirForm, NodeKind, OpaqueOperand, SubtypeRelation, TypeId};

use super::common::{TestResult, build, context};

fn symbol(name: &str) -> Literal {
    Literal::Symbol(SymbolRef::interned("COMMON-LISP", name))
}

fn list(items: impl IntoIterator<Item = Literal>) -> Literal {
    items
        .into_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .fold(Literal::Nil, |tail, head| {
            Literal::Cons(Box::new(head), Box::new(tail))
        })
}

fn compound(name: &str, operands: impl IntoIterator<Item = Literal>) -> Literal {
    list(std::iter::once(symbol(name)).chain(operands))
}

fn the(specifier: Literal) -> Expr {
    Expr::The {
        type_specifier: TypeSpecifier::new(specifier),
        value: Box::new(Expr::Constant(Literal::fixnum(7))),
    }
}

fn asserted_types(form: &HirForm) -> TestResult<Vec<(TypeId, ClTypeKind)>> {
    form.nodes()
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            NodeKind::TypeAssertion { expected_type, .. } => Some(*expected_type),
            _ => None,
        })
        .map(|id| {
            form.types()
                .get(id)
                .map(|ty| (id, ty.kind().clone()))
                .ok_or_else(|| {
                    std::io::Error::other("type assertion references a missing type").into()
                })
        })
        .collect()
}

fn at<T>(items: &[T], index: usize) -> TestResult<&T> {
    items
        .get(index)
        .ok_or_else(|| std::io::Error::other("expected type result is missing").into())
}

#[test]
fn atomic_and_boolean_type_specifiers_build_their_set_operations() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(53);
    let expr = Expr::Progn(vec![
        the(symbol("T")),
        the(symbol("NIL")),
        the(symbol("INTEGER")),
        the(compound("OR", [symbol("INTEGER"), symbol("STRING")])),
        the(compound("AND", [symbol("INTEGER"), symbol("NUMBER")])),
        the(compound("NOT", [symbol("NIL")])),
    ]);
    let form = build(
        &mut builder,
        id,
        &expr,
        "atomic and boolean type specifiers",
    )?;
    let actual = asserted_types(&form)?;

    // CLHS 4.2.3 defines atomic names and OR, AND, and NOT compound specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(at(&actual, 0)?.1, ClTypeKind::Any);
    assert_eq!(at(&actual, 1)?.1, ClTypeKind::Empty);
    assert_eq!(at(&actual, 2)?.1, ClTypeKind::Named(BuiltinType::Integer));
    assert!(matches!(&at(&actual, 3)?.1, ClTypeKind::Union(items) if items.len() == 2));
    assert!(matches!(&at(&actual, 4)?.1, ClTypeKind::Intersection(items) if items.len() == 2));
    assert!(matches!(&at(&actual, 5)?.1, ClTypeKind::Complement(_)));
    assert_eq!(
        form.types().subtype(at(&actual, 2)?.0, at(&actual, 3)?.0)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        form.types().subtype(at(&actual, 2)?.0, at(&actual, 4)?.0)?,
        SubtypeRelation::Yes
    );
    Ok(())
}

#[test]
fn member_integer_and_cons_specifiers_preserve_members_and_bounds() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(54);
    let member = compound(
        "MEMBER",
        [
            Literal::Nil,
            Literal::T,
            Literal::fixnum(3),
            Literal::Character(u32::from('A')),
            Literal::Number(NumberLiteral::SingleFloat(1.5)),
            Literal::Number(NumberLiteral::DoubleFloat(2.5)),
            Literal::Symbol(SymbolRef::interned("COMMON-LISP-USER", "VALUE")),
        ],
    );
    let expr = Expr::Progn(vec![
        the(member),
        the(compound("INTEGER", [])),
        the(compound("INTEGER", [symbol("*"), Literal::fixnum(10)])),
        the(compound("INTEGER", [Literal::fixnum(-2), symbol("*")])),
        the(compound("CONS", [])),
        the(compound("CONS", [symbol("INTEGER")])),
    ]);
    let form = build(
        &mut builder,
        id,
        &expr,
        "member, integer, and cons type specifiers",
    )?;
    let actual = asserted_types(&form)?;

    // CLHS 4.2.3 defines MEMBER, INTEGER bounds, and CONS compound specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    let ClTypeKind::Member(values) = &at(&actual, 0)?.1 else {
        return Err(std::io::Error::other("MEMBER did not become a finite member type").into());
    };
    assert_eq!(values.len(), 7);
    assert_eq!(at(values, 0)?, &ncl_hir::MemberValue::Nil);
    assert_eq!(at(values, 1)?, &ncl_hir::MemberValue::True);
    assert_eq!(at(values, 2)?, &ncl_hir::MemberValue::Integer(3));
    assert_eq!(at(values, 3)?, &ncl_hir::MemberValue::Character('A'));
    assert_eq!(
        at(values, 4)?,
        &ncl_hir::MemberValue::SingleFloat(1.5_f32.to_bits())
    );
    assert_eq!(
        at(values, 5)?,
        &ncl_hir::MemberValue::DoubleFloat(2.5_f64.to_bits())
    );
    let ncl_hir::MemberValue::Symbol(member_symbol) = at(values, 6)? else {
        return Err(
            std::io::Error::other("symbol member did not retain its symbol identity").into(),
        );
    };
    assert_eq!(
        builder
            .symbols()
            .get(*member_symbol)
            .map(|item| item.name()),
        Some("VALUE")
    );
    assert_eq!(at(&actual, 1)?.1, ClTypeKind::Named(BuiltinType::Integer));
    let ClTypeKind::NumericRange(lower_bounded) = &at(&actual, 2)?.1 else {
        return Err(std::io::Error::other("bounded INTEGER did not become a numeric range").into());
    };
    assert_eq!(lower_bounded.lower(), None);
    assert_eq!(
        lower_bounded.upper().map(|bound| bound.value()),
        Some(ncl_hir::NumericValue::Integer(10))
    );
    let ClTypeKind::NumericRange(upper_bounded) = &at(&actual, 3)?.1 else {
        return Err(
            std::io::Error::other("lower-bounded INTEGER did not become a numeric range").into(),
        );
    };
    assert_eq!(
        upper_bounded.lower().map(|bound| bound.value()),
        Some(ncl_hir::NumericValue::Integer(-2))
    );
    assert_eq!(upper_bounded.upper(), None);
    assert!(matches!(&at(&actual, 4)?.1, ClTypeKind::Cons { .. }));
    assert!(matches!(&at(&actual, 5)?.1, ClTypeKind::Cons { .. }));
    Ok(())
}

#[test]
fn unsupported_type_forms_keep_their_original_source_constants() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(55);
    let foreign_integer = Literal::Symbol(SymbolRef::interned("COMMON-LISP-USER", "INTEGER"));
    let unknown = compound("CUSTOM-TYPE", [Literal::fixnum(9)]);
    let malformed_not = compound("NOT", [symbol("INTEGER"), symbol("STRING")]);
    let malformed_integer = compound("INTEGER", [Literal::fixnum(1)]);
    let unsupported_member = compound("MEMBER", [Literal::String(vec!['x'])]);
    let expr = Expr::Progn(vec![
        the(foreign_integer),
        the(unknown),
        the(malformed_not),
        the(malformed_integer),
        the(unsupported_member),
    ]);
    let form = build(&mut builder, id, &expr, "unsupported type specifiers")?;
    let actual = asserted_types(&form)?;

    // CLHS 4.2.3 permits user-defined type specifiers; unknown meanings remain represented as source data.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    for (_, kind) in actual {
        let ClTypeKind::Opaque {
            specifier,
            operands,
        } = kind
        else {
            return Err(std::io::Error::other(
                "unsupported type specifier was not retained as opaque",
            )
            .into());
        };
        assert!(builder.symbols().get(specifier).is_some());
        assert!(matches!(operands.as_slice(), [OpaqueOperand::Constant(_)]));
    }
    Ok(())
}

#[test]
fn eql_empty_boolean_and_invalid_ranges_keep_their_type_meaning() -> TestResult<()> {
    let (id, mut builder, _fixture) = context(56);
    let eql = compound("EQL", [Literal::fixnum(7)]);
    let empty_or = compound("OR", []);
    let empty_and = compound("AND", []);
    let malformed_eql = compound("EQL", [Literal::fixnum(7), Literal::fixnum(8)]);
    let reversed_integer = compound("INTEGER", [Literal::fixnum(8), Literal::fixnum(2)]);
    let invalid_character = compound("MEMBER", [Literal::Character(0x11_0000)]);
    let expr = Expr::Progn(vec![
        the(eql),
        the(empty_or),
        the(empty_and),
        the(malformed_eql),
        the(reversed_integer),
        the(invalid_character),
    ]);
    let form = build(&mut builder, id, &expr, "EQL and empty type specifiers")?;
    let actual = asserted_types(&form)?;

    // CLHS 4.2.3 defines EQL, OR, AND, and bounded INTEGER type specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(
        at(&actual, 0)?.1,
        ClTypeKind::Member(vec![ncl_hir::MemberValue::Integer(7)])
    );
    assert_eq!(at(&actual, 1)?.1, ClTypeKind::Empty);
    assert_eq!(at(&actual, 2)?.1, ClTypeKind::Any);
    for (_, kind) in actual.into_iter().skip(3) {
        assert!(matches!(kind, ClTypeKind::Opaque { .. }));
    }
    Ok(())
}

use super::super::*;

#[test]
fn member_values_reach_float_union_and_named_relation_cases() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(126))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let atom = named(&mut arena, BuiltinType::Atom)?;
    let symbol_type = named(&mut arena, BuiltinType::Symbol)?;
    let null = named(&mut arena, BuiltinType::Null)?;
    let single_range = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let single_member = arena.member(vec![MemberValue::SingleFloat(0.5_f32.to_bits())])?;
    let double_member = arena.member(vec![MemberValue::DoubleFloat(0.5_f64.to_bits())])?;
    let double_range = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.0_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;
    let character_member = arena.member(vec![MemberValue::Character('x')])?;
    let true_member = arena.member(vec![MemberValue::True])?;
    let true_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 8)?;
    let symbol_member = arena.member(vec![MemberValue::Symbol(true_symbol)])?;
    let integer_member = arena.member(vec![MemberValue::Integer(1)])?;
    let nil_member = arena.member(vec![MemberValue::Nil])?;
    let member_super_union = arena.union([integer, character])?;
    let nested_super = arena.intersection([member_super_union, atom])?;
    let not_single_member = arena.complement(single_member)?;
    let not_true = arena.member(vec![MemberValue::True])?;
    let not_double_member = arena.complement(double_member)?;
    let not_character_member = arena.complement(character_member)?;
    let not_integer = arena.complement(integer)?;
    let not_character = arena.complement(character)?;
    let not_atom = arena.complement(atom)?;

    // CLHS MEMBER uses EQL values; Common Lisp numeric format and named type rules remain distinct.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(
        arena.subtype(single_member, single_range)?,
        SubtypeRelation::Yes
    );
    // The lattice does not prove disjointness for a cross-format bounded range.
    assert_eq!(
        arena.subtype(double_member, single_range)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(character_member, single_range)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(double_member, double_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(true_member, symbol_type)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(nil_member, integer)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(true_member, null)?, SubtypeRelation::No);
    // CLHS numeric types and CHARACTER have disjoint values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_char.htm
    assert_eq!(arena.subtype(integer, character)?, SubtypeRelation::No);
    // ATOM includes INTEGER, so an integer is excluded by the complement of ATOM.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_atom.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_not.htm
    assert_eq!(arena.subtype(integer, not_atom)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(null, not_true)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(symbol_type, symbol_member)?,
        SubtypeRelation::No
    );
    // The HIR contract preserves uncertainty where finite EQL facts cannot prove inclusion.
    // docs/src/design/hir.md
    assert_eq!(
        arena.subtype(integer, integer_member)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(integer_member, nested_super)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(character, not_integer)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(integer, not_character)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(single_range, not_single_member)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(single_range, not_double_member)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(single_range, not_character_member)?,
        SubtypeRelation::Yes
    );
    Ok(())
}

use std::error::Error;

use super::{
    ArrayDimensions, ArrayType, BuiltinType, MemberValue, NumericBound, NumericDomain,
    NumericRange, NumericValue, OpaqueOperand, SubtypeRelation, TypeArena, form, named,
};
use ncl_hir::ids::{CompilationUnitId, SymbolId, TypeId};

fn member(
    arena: &mut TypeArena,
    values: impl IntoIterator<Item = MemberValue>,
) -> Result<TypeId, Box<dyn Error>> {
    Ok(arena.member(values.into_iter().collect())?)
}

#[test]
fn finite_member_values_must_all_fit_numeric_and_named_types() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(109))?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let keyword = named(&mut arena, BuiltinType::Keyword)?;
    let range = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let outside_range = arena.complement(range)?;
    let one = member(&mut arena, [MemberValue::Integer(1)])?;
    let one_and_two = member(
        &mut arena,
        [MemberValue::Integer(1), MemberValue::Integer(2)],
    )?;
    let one_and_three = member(
        &mut arena,
        [MemberValue::Integer(1), MemberValue::Integer(3)],
    )?;
    let three_and_four = member(
        &mut arena,
        [MemberValue::Integer(3), MemberValue::Integer(4)],
    )?;
    let opaque_and_four = member(
        &mut arena,
        [
            MemberValue::OpaqueObject {
                owner: CompilationUnitId::new(0),
                index: 5,
            },
            MemberValue::Integer(4),
        ],
    )?;
    let integer_and_character = member(
        &mut arena,
        [MemberValue::Integer(1), MemberValue::Character('x')],
    )?;
    let symbol = member(
        &mut arena,
        [MemberValue::Symbol(ncl_hir::ids::SymbolId::try_from_index(
            CompilationUnitId::new(0),
            7,
        )?)],
    )?;

    // CLHS MEMBER denotes the listed EQL values, while numeric type specifiers
    // constrain every value in the set. An opaque HIR object cannot prove overlap.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(one, range)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(one_and_two, range)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(one_and_three, range)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(three_and_four, outside_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(one_and_three, outside_range)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(opaque_and_four, outside_range)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(one_and_two, number)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(integer_and_character, number)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(symbol, keyword)?, SubtypeRelation::Unknown);
    assert_eq!(arena.subtype(one_and_two, integer)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn numeric_and_array_relations_stay_unknown_without_a_proof() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(110))?;
    let single_float = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let double_float = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.0_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;
    let not_double_float = arena.complement(double_float)?;
    let rank_unknown_array = arena.array(ArrayType {
        element_type: None,
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;
    let vector = named(&mut arena, BuiltinType::Vector)?;
    let not_vector = arena.complement(vector)?;

    // CLHS defines these numeric formats and compound array constraints. HIR
    // keeps the result unknown where its model cannot prove subtyping or disjointness.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(
        arena.subtype(single_float, not_double_float)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(rank_unknown_array, vector)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(rank_unknown_array, not_vector)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

#[test]
fn finite_member_overlap_uses_eql_and_preserves_symbol_uncertainty() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(111))?;
    let one = member(&mut arena, [MemberValue::Integer(1)])?;
    let one_or_two = member(
        &mut arena,
        [MemberValue::Integer(1), MemberValue::Integer(2)],
    )?;
    let two = member(&mut arena, [MemberValue::Integer(2)])?;
    let nil_or_true = member(&mut arena, [MemberValue::Nil, MemberValue::True])?;
    let symbol = member(
        &mut arena,
        [MemberValue::Symbol(SymbolId::try_from_index(
            CompilationUnitId::new(0),
            0,
        )?)],
    )?;

    // CLHS 4.2.3 lists MEMBER as a standardized compound type specifier; its
    // entry defines membership by EQL comparison.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    assert_eq!(arena.subtype(one, one_or_two)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(one, two)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(symbol, nil_or_true)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(nil_or_true, symbol)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

#[test]
fn boolean_and_null_relations_match_nil_and_true_members() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(112))?;
    let boolean = named(&mut arena, BuiltinType::Boolean)?;
    let null = named(&mut arena, BuiltinType::Null)?;
    let symbol = named(&mut arena, BuiltinType::Symbol)?;
    let nil = member(&mut arena, [MemberValue::Nil])?;
    let true_value = member(&mut arena, [MemberValue::True])?;
    let nil_or_true = member(&mut arena, [MemberValue::Nil, MemberValue::True])?;
    let unknown_symbol = member(
        &mut arena,
        [MemberValue::Symbol(SymbolId::try_from_index(
            CompilationUnitId::new(0),
            1,
        )?)],
    )?;

    // CLHS defines NULL as the singleton NIL type and BOOLEAN as the set {NIL, T}.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_null.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_ban.htm
    assert_eq!(arena.subtype(null, nil)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(nil, boolean)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(true_value, boolean)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(boolean, nil_or_true)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(boolean, nil)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(boolean, null)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(nil_or_true, symbol)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(unknown_symbol, boolean)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

#[test]
fn compound_types_prove_not_disjointness_only_when_every_case_is_known()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(113))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let not_number = arena.complement(number)?;
    let disjoint_union = arena.union([character, string])?;
    let overlapping_union = arena.union([integer, character])?;
    let opaque = arena.opaque(
        SymbolId::try_from_index(CompilationUnitId::new(0), 2)?,
        [OpaqueOperand::Type(integer)],
    )?;
    let uncertain_union = arena.union([character, opaque])?;
    let disjoint_intersection = arena.intersection([character, number])?;
    let uncertain_intersection = arena.intersection([integer, number])?;

    // CLHS 4.2.3 defines OR, AND, and NOT as union, intersection, and complement.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_or.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_and.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_not.htm
    assert_eq!(
        arena.subtype(disjoint_union, not_number)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(overlapping_union, not_number)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(uncertain_union, not_number)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(disjoint_intersection, not_number)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(uncertain_intersection, not_number)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

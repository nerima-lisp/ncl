use std::error::Error;

use super::{
    ArrayDimensions, ArrayType, BuiltinType, MemberValue, NumericBound, NumericDomain,
    NumericRange, NumericValue, SubtypeRelation, TypeArena, form, named,
};
use ncl_hir::ids::{CompilationUnitId, TypeId};

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

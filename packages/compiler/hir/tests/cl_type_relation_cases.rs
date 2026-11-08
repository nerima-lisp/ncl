//! Behavioral cases for finite-value and numeric type relations.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::error::Error;

use ncl_hir::cl_type::{
    ArrayDimensions, ArrayType, BuiltinType, FunctionParameters, FunctionType, MemberValue,
    NumericBound, NumericDomain, NumericRange, NumericValue, SubtypeRelation, TypeArena,
};
use ncl_hir::ids::{CompilationUnitId, FormId};

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(0), index)
}

#[test]
fn float_member_types_compare_eql_values_and_keep_formats_distinct() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(300))?;
    let positive_zero = arena.member(vec![MemberValue::SingleFloat(0.0_f32.to_bits())])?;
    let negative_zero = arena.member(vec![MemberValue::SingleFloat((-0.0_f32).to_bits())])?;
    let same_value = arena.member(vec![MemberValue::SingleFloat(0.0_f32.to_bits())])?;
    let one = arena.member(vec![MemberValue::SingleFloat(1.0_f32.to_bits())])?;
    let double_zero = arena.member(vec![MemberValue::DoubleFloat(0.0_f64.to_bits())])?;
    let not_one = arena.complement(one)?;
    let not_double_zero = arena.complement(double_zero)?;
    let not_same_value = arena.complement(same_value)?;

    // CLHS MEMBER compares values by EQL; EQL distinguishes float formats.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(positive_zero, negative_zero);
    assert_eq!(
        arena.subtype(positive_zero, negative_zero)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(positive_zero, not_one)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(positive_zero, not_same_value)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(positive_zero, not_double_zero)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(positive_zero, double_zero)?,
        SubtypeRelation::No
    );
    Ok(())
}

#[test]
fn finite_numeric_members_do_not_match_cons_array_or_function_objects() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(301))?;
    let integer = arena.named(BuiltinType::Integer)?;
    let any = arena.any();
    let nil = arena.named(BuiltinType::Null)?;
    let integer_member = arena.member(vec![MemberValue::Integer(7)])?;
    let opaque_member = arena.member(vec![MemberValue::OpaqueObject {
        owner: CompilationUnitId::new(0),
        index: 4,
    }])?;
    let cons = arena.cons(any, nil)?;
    let array = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;
    let function = arena.function(FunctionType {
        parameters: FunctionParameters::Any,
        returns: any,
    })?;

    // CLHS defines these object type families; an opaque member's structure is not modeled by HIR.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    // docs/src/design/hir.md:132-136
    for structural_type in [cons, array, function] {
        assert_eq!(
            arena.subtype(integer_member, structural_type)?,
            SubtypeRelation::No
        );
        assert_eq!(
            arena.subtype(opaque_member, structural_type)?,
            SubtypeRelation::Unknown
        );
    }
    Ok(())
}

#[test]
fn signed_integer_interval_limits_preserve_exclusive_and_inclusive_endpoints()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(302))?;
    let low = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::exclusive(NumericValue::Integer(i64::MIN))),
        Some(NumericBound::inclusive(NumericValue::Integer(i64::MIN + 1))),
    )?)?;
    let low_member = arena.member(vec![MemberValue::Integer(i64::MIN + 1)])?;
    let low_excluded = arena.member(vec![MemberValue::Integer(i64::MIN)])?;
    let high = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(i64::MAX - 1))),
        Some(NumericBound::exclusive(NumericValue::Integer(i64::MAX))),
    )?)?;
    let high_member = arena.member(vec![MemberValue::Integer(i64::MAX - 1)])?;
    let high_excluded = arena.member(vec![MemberValue::Integer(i64::MAX)])?;

    // CLHS INTEGER interval designators include or exclude each stated endpoint.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(low_member, low)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(low_excluded, low)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(low, low_member)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(high_member, high)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(high_excluded, high)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(high, high_member)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn integer_ranges_leave_unclassified_character_relations_unknown() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(303))?;
    let character = arena.named(BuiltinType::Character)?;
    let integer_range = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?)?;

    // CLHS distinguishes numeric and character type specifiers. The HIR lattice
    // preserves an unknown relation when no rule proves either direction.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    // docs/src/design/hir.md:132-136
    assert_eq!(
        arena.subtype(integer_range, character)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(character, integer_range)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

//! Behavioral edge coverage for the Common Lisp type lattice.

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
    ArrayDimensions, ArrayType, BuiltinType, MemberValue, NumericBound, NumericDomain,
    NumericRange, NumericValue, SubtypeRelation, TypeArena,
};
use ncl_hir::ids::{CompilationUnitId, FormId, SymbolId};

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(0), index)
}

#[test]
fn empty_and_singleton_types_reuse_their_canonical_identities() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(200))?;
    let empty = arena.empty();
    let any = arena.any();

    // HIR represents the empty type and canonicalizes OR/AND forms.
    // docs/src/design/hir.md:122-126
    // CLHS 4.2.3 defines OR and AND type specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.len(), 2);
    assert!(!arena.is_empty());
    assert_eq!(arena.member(Vec::new())?, empty);
    assert_eq!(arena.len(), 2);

    let integer = arena.named(BuiltinType::Integer)?;
    let after_integer = arena.len();
    assert_eq!(arena.union([integer])?, integer);
    assert_eq!(arena.intersection([any, integer])?, integer);
    assert_eq!(arena.len(), after_integer);

    // The empty set and singleton boolean forms are canonical HIR types.
    Ok(())
}

#[test]
fn numeric_ranges_distinguish_overlap_disjointness_and_unknown_formats()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(201))?;
    let left = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let right = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(3))),
        Some(NumericBound::inclusive(NumericValue::Integer(4))),
    )?)?;
    let overlap = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
        Some(NumericBound::inclusive(NumericValue::Integer(3))),
    )?)?;
    let not_right = arena.complement(right)?;
    let not_overlap = arena.complement(overlap)?;

    // CLHS integer type intervals include or exclude endpoints as specified.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(left, not_right)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(left, not_overlap)?, SubtypeRelation::No);

    let single = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let double = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.0_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;
    let not_double = arena.complement(double)?;
    let not_single = arena.complement(single)?;
    assert_eq!(arena.subtype(single, not_double)?, SubtypeRelation::Unknown);
    assert_eq!(arena.subtype(left, not_single)?, SubtypeRelation::Yes);

    let integer_member = arena.member(vec![MemberValue::Integer(1)])?;
    let single_member = arena.member(vec![MemberValue::SingleFloat(0.5_f32.to_bits())])?;
    assert_eq!(
        arena.subtype(integer_member, single)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(single_member, single)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn singleton_float_ranges_match_their_exact_member_values() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(202))?;
    let single_bits = (-1.0_f32).to_bits();
    let single_point = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            single_bits,
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            single_bits,
        ))),
    )?)?;
    let single_member = arena.member(vec![MemberValue::SingleFloat(single_bits)])?;
    let double_bits = (-2.0_f64).to_bits();
    let double_point = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            double_bits,
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            double_bits,
        ))),
    )?)?;
    let double_member = arena.member(vec![MemberValue::DoubleFloat(double_bits)])?;

    // CLHS MEMBER compares objects with EQL, including the exact float value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(
        arena.subtype(single_point, single_member)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_point, double_member)?,
        SubtypeRelation::Yes
    );

    let interval = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            single_bits,
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
    )?)?;
    assert_eq!(arena.subtype(interval, single_member)?, SubtypeRelation::No);
    Ok(())
}

#[test]
fn finite_values_keep_named_union_and_complement_relations_conservative()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(203))?;
    let symbol_id = SymbolId::try_from_index(CompilationUnitId::new(0), 1)?;
    let symbol = arena.member(vec![MemberValue::Symbol(symbol_id)])?;
    let integer_member = arena.member(vec![MemberValue::Integer(7)])?;
    let true_member = arena.member(vec![MemberValue::True])?;
    let opaque_member = arena.member(vec![MemberValue::OpaqueObject {
        owner: CompilationUnitId::new(0),
        index: 2,
    }])?;

    let integer = arena.named(BuiltinType::Integer)?;
    let symbol_type = arena.named(BuiltinType::Symbol)?;
    let boolean = arena.named(BuiltinType::Boolean)?;
    let character = arena.named(BuiltinType::Character)?;
    let number = arena.named(BuiltinType::Number)?;
    let symbol_or_integer = arena.union([integer, symbol_type])?;
    let integer_and_number = arena.intersection([integer, number])?;
    let not_integer = arena.complement(integer)?;
    let not_character = arena.complement(character)?;
    let not_number = arena.complement(number)?;
    let character_member = arena.member(vec![MemberValue::Character('x')])?;

    // CLHS MEMBER uses EQL values; unresolved symbol identity stays conservative.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(
        arena.subtype(symbol, symbol_or_integer)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(integer_member, integer_and_number)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(true_member, boolean)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(symbol, boolean)?, SubtypeRelation::Unknown);
    assert_eq!(
        arena.subtype(integer_member, not_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(character_member, not_integer)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(character_member, not_character)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(opaque_member, not_number)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

#[test]
fn floating_members_follow_their_common_lisp_numeric_type_families() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(205))?;
    let single_member = arena.member(vec![MemberValue::SingleFloat(0.5_f32.to_bits())])?;
    let double_member = arena.member(vec![MemberValue::DoubleFloat(0.5_f64.to_bits())])?;
    let single_float = arena.named(BuiltinType::SingleFloat)?;
    let double_float = arena.named(BuiltinType::DoubleFloat)?;
    let float = arena.named(BuiltinType::Float)?;
    let real = arena.named(BuiltinType::Real)?;
    let number = arena.named(BuiltinType::Number)?;
    let integer = arena.named(BuiltinType::Integer)?;

    // CLHS 4.2.3 defines the distinct float formats and their numeric super-types.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    for supertype in [single_float, float, real, number] {
        assert_eq!(
            arena.subtype(single_member, supertype)?,
            SubtypeRelation::Yes
        );
    }
    for supertype in [double_float, float, real, number] {
        assert_eq!(
            arena.subtype(double_member, supertype)?,
            SubtypeRelation::Yes
        );
    }
    assert_eq!(
        arena.subtype(single_member, double_float)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(double_member, single_float)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(single_member, integer)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(double_member, integer)?, SubtypeRelation::No);

    let single_range = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.5_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.5_f32.to_bits(),
        ))),
    )?)?;
    let double_range = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.5_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.5_f64.to_bits(),
        ))),
    )?)?;
    assert_eq!(
        arena.subtype(single_member, single_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_member, double_range)?,
        SubtypeRelation::Yes
    );
    Ok(())
}

#[test]
fn float_intervals_order_negative_values_and_include_both_closed_endpoints()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(206))?;
    let single_interval = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            (-1.0_f32).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let double_interval = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-1.0_f64).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;
    let single_below = arena.member(vec![MemberValue::SingleFloat((-2.0_f32).to_bits())])?;
    let single_lower = arena.member(vec![MemberValue::SingleFloat((-1.0_f32).to_bits())])?;
    let single_zero = arena.member(vec![MemberValue::SingleFloat(0.0_f32.to_bits())])?;
    let single_upper = arena.member(vec![MemberValue::SingleFloat(1.0_f32.to_bits())])?;
    let single_above = arena.member(vec![MemberValue::SingleFloat(2.0_f32.to_bits())])?;
    let double_below = arena.member(vec![MemberValue::DoubleFloat((-2.0_f64).to_bits())])?;
    let double_lower = arena.member(vec![MemberValue::DoubleFloat((-1.0_f64).to_bits())])?;
    let double_zero = arena.member(vec![MemberValue::DoubleFloat(0.0_f64.to_bits())])?;
    let double_upper = arena.member(vec![MemberValue::DoubleFloat(1.0_f64.to_bits())])?;
    let double_above = arena.member(vec![MemberValue::DoubleFloat(2.0_f64.to_bits())])?;

    // CLHS numeric type intervals include both stated endpoints here.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    for (member, expected) in [
        (single_below, SubtypeRelation::No),
        (single_lower, SubtypeRelation::Yes),
        (single_zero, SubtypeRelation::Yes),
        (single_upper, SubtypeRelation::Yes),
        (single_above, SubtypeRelation::No),
    ] {
        assert_eq!(arena.subtype(member, single_interval)?, expected);
    }
    for (member, expected) in [
        (double_below, SubtypeRelation::No),
        (double_lower, SubtypeRelation::Yes),
        (double_zero, SubtypeRelation::Yes),
        (double_upper, SubtypeRelation::Yes),
        (double_above, SubtypeRelation::No),
    ] {
        assert_eq!(arena.subtype(member, double_interval)?, expected);
    }
    Ok(())
}

#[test]
fn same_format_float_ranges_prove_containment_and_disjointness() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(207))?;
    let single_outer = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            3.0_f32.to_bits(),
        ))),
    )?)?;
    let single_inner = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            2.0_f32.to_bits(),
        ))),
    )?)?;
    let single_after = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            4.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            5.0_f32.to_bits(),
        ))),
    )?)?;
    let double_outer = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-3.0_f64).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-1.0_f64).to_bits(),
        ))),
    )?)?;
    let double_inner = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-2.0_f64).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-1.5_f64).to_bits(),
        ))),
    )?)?;
    let double_before = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-5.0_f64).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            (-4.0_f64).to_bits(),
        ))),
    )?)?;
    let not_single_after = arena.complement(single_after)?;
    let not_double_before = arena.complement(double_before)?;

    // CLHS 4.2.3 distinguishes numeric type specifiers and interval endpoints.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(
        arena.subtype(single_inner, single_outer)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(single_outer, single_inner)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(single_outer, not_single_after)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_inner, double_outer)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_outer, not_double_before)?,
        SubtypeRelation::Yes
    );
    Ok(())
}

#[test]
fn array_axis_constraints_match_each_dimension_and_preserve_vector_relations()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(204))?;
    let integer = arena.named(BuiltinType::Integer)?;
    let number = arena.named(BuiltinType::Number)?;
    let axes = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![Some(4), None]),
        simple: true,
    })?;
    let same_axes = arena.array(ArrayType {
        element_type: Some(number),
        dimensions: ArrayDimensions::Axes(vec![Some(4), None]),
        simple: false,
    })?;
    let fixed_second_axis = arena.array(ArrayType {
        element_type: Some(number),
        dimensions: ArrayDimensions::Axes(vec![Some(4), Some(3)]),
        simple: false,
    })?;
    let one_dimensional = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![Some(4)]),
        simple: true,
    })?;
    let two_dimensional = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(2),
        simple: true,
    })?;
    let vector = arena.named(BuiltinType::Vector)?;

    // CLHS ARRAY dimensions constrain rank and each specified axis.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_array.htm
    assert_eq!(arena.subtype(axes, same_axes)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(axes, fixed_second_axis)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(one_dimensional, vector)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(two_dimensional, vector)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

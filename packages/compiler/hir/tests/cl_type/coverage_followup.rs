use std::error::Error;

use super::{
    ArrayDimensions, ArrayType, BuiltinType, ClTypeError, FunctionParameters, FunctionType,
    MemberValue, NumericBound, NumericDomain, NumericRange, NumericRangeError, NumericValue,
    OpaqueOperand, SubtypeRelation, TypeArena, form, named,
};
use ncl_hir::ids::{CompilationUnitId, IdError, SymbolId, TypeId};

#[test]
fn nested_intersections_flatten_to_the_same_type() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(120))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let real = named(&mut arena, BuiltinType::Real)?;
    let inner = arena.intersection([integer, number])?;
    let nested = arena.intersection([inner, real])?;
    let flat = arena.intersection([integer, number, real])?;

    assert_eq!(nested, flat);
    Ok(())
}

#[test]
fn public_type_constructors_validate_child_identities() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(121))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let foreign_type = TypeId::try_from_index(form(122), 7)?;
    let local_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 3)?;
    let foreign_symbol = SymbolId::try_from_index(CompilationUnitId::new(1), 4)?;

    assert_eq!(
        arena.union([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.intersection([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.cons(foreign_type, integer),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.cons(integer, foreign_type),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.array(ArrayType {
            element_type: Some(foreign_type),
            dimensions: ArrayDimensions::Any,
            simple: false,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.function(FunctionType {
            parameters: FunctionParameters::Fixed(vec![foreign_type]),
            returns: integer,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.function(FunctionType {
            parameters: FunctionParameters::Any,
            returns: foreign_type,
        }),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.multiple_values([foreign_type]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.opaque(foreign_symbol, []),
        Err(ClTypeError::ForeignSymbol(foreign_symbol))
    );
    assert_eq!(
        arena.opaque(local_symbol, [OpaqueOperand::Type(foreign_type)]),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    assert_eq!(
        arena.opaque(local_symbol, [OpaqueOperand::Symbol(foreign_symbol)]),
        Err(ClTypeError::ForeignSymbol(foreign_symbol))
    );
    assert_eq!(
        arena.member(vec![MemberValue::OpaqueObject {
            owner: CompilationUnitId::new(1),
            index: 5,
        }]),
        Err(ClTypeError::ForeignObject(CompilationUnitId::new(1)))
    );

    let error = ClTypeError::UnknownType(foreign_type);
    assert!(error.to_string().starts_with("HIR type error:"));
    let id_error = IdError::IndexOverflow { index: usize::MAX };
    assert_eq!(ClTypeError::from(id_error), ClTypeError::Id(id_error));
    Ok(())
}

#[test]
fn numeric_bound_flags_and_range_errors_are_observable() {
    let included = NumericBound::inclusive(NumericValue::Integer(2));
    let excluded = NumericBound::exclusive(NumericValue::Integer(2));

    assert!(included.is_inclusive());
    assert!(!excluded.is_inclusive());
    assert_eq!(included.value(), NumericValue::Integer(2));
    assert_eq!(excluded.value(), NumericValue::Integer(2));
    assert!(format!("{}", NumericRangeError::EmptyInterval).starts_with("numeric range error:"));
}

#[test]
fn numeric_ranges_cover_named_formats_and_order_both_interval_directions()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(123))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let single = named(&mut arena, BuiltinType::SingleFloat)?;
    let double = named(&mut arena, BuiltinType::DoubleFloat)?;
    let rational = named(&mut arena, BuiltinType::Rational)?;
    let complex = named(&mut arena, BuiltinType::Complex)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let full_integer =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let upper_bounded_integer = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        None,
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?)?;
    let bounded_integer = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(0))),
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?)?;
    let full_single =
        arena.numeric_range(NumericRange::new(NumericDomain::SingleFloat, None, None)?)?;
    let bounded_single = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let full_double =
        arena.numeric_range(NumericRange::new(NumericDomain::DoubleFloat, None, None)?)?;
    let bounded_double = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.0_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;

    // CLHS defines the standard numeric type families and their interval bounds.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_float.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_complex.htm
    // docs/src/design/hir.md: unresolved relations remain Unknown.
    assert_eq!(arena.subtype(integer, full_integer)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(integer, bounded_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(single, full_single)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(single, bounded_single)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(double, full_double)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(double, bounded_double)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(bounded_single, integer)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_single, rational)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(bounded_single, complex)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_single, string)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(bounded_double, integer)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_double, rational)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(bounded_double, complex)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_double, string)?,
        SubtypeRelation::Unknown
    );

    assert_float_interval_subtyping(&mut arena)?;

    assert_eq!(
        arena.subtype(full_integer, bounded_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(full_integer, upper_bounded_integer)?,
        SubtypeRelation::No
    );
    Ok(())
}

fn assert_float_interval_subtyping(arena: &mut TypeArena) -> Result<(), Box<dyn Error>> {
    let low = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let high = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::exclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            2.0_f32.to_bits(),
        ))),
    )?)?;
    let not_low = arena.complement(low)?;
    let not_high = arena.complement(high)?;
    assert_eq!(arena.subtype(low, not_high)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(high, not_low)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn nonsingleton_ranges_and_finite_members_keep_exact_overlap_results() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(124))?;
    let all_integers =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let interval = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let singleton = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
    )?)?;
    let one = arena.member(vec![MemberValue::Integer(1)])?;
    let two = arena.member(vec![MemberValue::Integer(2)])?;
    let not_one = arena.complement(one)?;

    // CLHS MEMBER is the set of listed EQL values; numeric interval endpoints are inclusive here.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(arena.subtype(all_integers, one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(interval, one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(singleton, one)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(singleton, two)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(interval, not_one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(all_integers, not_one)?, SubtypeRelation::No);
    Ok(())
}

#[test]
fn array_relation_respects_unbounded_elements_and_axis_shapes() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(125))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let typed_rank_one = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let untyped_rank_one = arena.array(ArrayType {
        element_type: None,
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let any_dimensions = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;
    let typed_rank_two = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(2),
        simple: false,
    })?;
    let number_rank_one = arena.array(ArrayType {
        element_type: Some(number),
        dimensions: ArrayDimensions::Rank(1),
        simple: false,
    })?;
    let axes_two = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![None, None]),
        simple: false,
    })?;
    let constrained_axis = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![Some(4)]),
        simple: false,
    })?;
    let unconstrained_axis = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![None]),
        simple: false,
    })?;

    // CLHS array type specifiers constrain element type, rank, and dimensions.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_array.htm
    assert_eq!(
        arena.subtype(typed_rank_one, untyped_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, any_dimensions)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, typed_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, number_rank_one)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(typed_rank_one, typed_rank_two)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(axes_two, typed_rank_one)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(typed_rank_one, constrained_axis)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(axes_two, unconstrained_axis)?,
        SubtypeRelation::No
    );
    Ok(())
}

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

#[test]
fn subtype_cache_union_and_double_complement_paths_preserve_contract() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(127))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let integer_cons = arena.cons(integer, integer)?;
    let number_cons = arena.cons(number, number)?;
    let not_integer = arena.complement(integer)?;
    let not_number = arena.complement(number)?;
    let not_character = arena.complement(character)?;
    let no_match = arena.union([character, string])?;
    let foreign = TypeId::try_from_index(form(128), 9)?;
    let class_symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 12)?;
    let class = arena.class(class_symbol)?;

    // CLHS defines OR and NOT set semantics; unresolved HIR facts stay Unknown by contract.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_or.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_not.htm
    // docs/src/design/hir.md
    assert_eq!(
        arena.subtype(integer_cons, number_cons)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(integer, no_match)?, SubtypeRelation::Unknown);
    assert_eq!(arena.subtype(string, not_character)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(not_integer, not_number)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(class, class)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(foreign, integer),
        Err(ClTypeError::UnknownType(foreign))
    );
    assert_eq!(
        arena.subtype(integer, foreign),
        Err(ClTypeError::UnknownType(foreign))
    );
    Ok(())
}

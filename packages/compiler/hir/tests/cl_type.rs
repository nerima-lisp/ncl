//! Tests for the Common Lisp type lattice.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]
#![expect(
    unused_crate_dependencies,
    reason = "This target includes source modules by path to test private invariants"
)]

#[path = "../src/arena.rs"]
/// The typed arena used by the type model.
pub mod arena;
#[path = "../src/cl_type.rs"]
/// The compiler-owned Common Lisp type lattice.
pub mod cl_type;
#[path = "../src/ids.rs"]
/// Typed identities used by the HIR arenas.
pub mod ids;

use std::error::Error;

use cl_type::{
    ArrayDimensions, ArrayType, BuiltinType, ClTypeError, ClTypeKind, FunctionParameters,
    FunctionType, MemberValue, NumericBound, NumericDomain, NumericRange, NumericRangeError,
    NumericValue, OpaqueOperand, SubtypeRelation, TypeArena,
};
use ids::{CompilationUnitId, FormId, SymbolId, TypeId, TypeSpecConstId};

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(0), index)
}

fn named(arena: &mut TypeArena, name: BuiltinType) -> Result<TypeId, ClTypeError> {
    arena.named(name)
}

#[test]
fn named_types_preserve_runtime_names_and_shared_subtype_meanings() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(0))?;
    let integer_name = BuiltinType::from_name("INTEGER");
    let real_name = BuiltinType::from_name("REAL");
    let string_name = BuiltinType::from_name("STRING");
    assert_eq!(integer_name, Some(BuiltinType::Integer));
    assert_eq!(real_name, Some(BuiltinType::Real));
    assert_eq!(string_name, Some(BuiltinType::String));
    assert_eq!(BuiltinType::from_name("IMPLEMENTATION-TYPE"), None);

    let integer = named(&mut arena, BuiltinType::Integer)?;
    let real = named(&mut arena, BuiltinType::Real)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let float = named(&mut arena, BuiltinType::Float)?;
    let cons = named(&mut arena, BuiltinType::Cons)?;
    let list = named(&mut arena, BuiltinType::List)?;
    let null = named(&mut arena, BuiltinType::Null)?;
    let sequence = named(&mut arena, BuiltinType::Sequence)?;
    assert_eq!(arena.subtype(integer, real)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(integer, string)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(integer, float)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(number, integer)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(string, number)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(cons, list)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(cons, sequence)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(null, list)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn boolean_types_flatten_sort_deduplicate_and_cache() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(1))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let duplicate_integer = named(&mut arena, BuiltinType::Integer)?;
    let real = named(&mut arena, BuiltinType::Real)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let any = arena.any();
    let empty = arena.empty();
    let nested = arena.union([string, integer])?;
    let union = arena.union([empty, nested, real])?;
    let same_union = arena.union([real, integer, string, integer])?;
    assert_eq!(union, same_union);
    assert_eq!(integer, duplicate_integer);
    assert_eq!(arena.union([union, any])?, any);
    assert_eq!(arena.union([])?, empty);

    let intersection = arena.intersection([real, integer, any])?;
    assert_eq!(arena.subtype(intersection, integer)?, SubtypeRelation::Yes);
    assert_eq!(arena.intersection([])?, any);
    assert_eq!(arena.intersection([integer, empty])?, empty);

    let negated = arena.complement(integer)?;
    assert_eq!(arena.complement(negated)?, integer);
    assert_eq!(arena.complement(any)?, empty);
    assert_eq!(arena.complement(empty)?, any);
    Ok(())
}

#[test]
fn numeric_ranges_keep_open_and_closed_bounds_and_three_way_results() -> Result<(), Box<dyn Error>>
{
    let outer = NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::exclusive(NumericValue::Integer(0))),
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?;
    let inner = NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::exclusive(NumericValue::Integer(10))),
    )?;
    let disjoint = NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
        Some(NumericBound::inclusive(NumericValue::Integer(12))),
    )?;
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(NumericBound::exclusive(NumericValue::Integer(3))),
            Some(NumericBound::inclusive(NumericValue::Integer(3))),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(NumericBound::exclusive(NumericValue::Integer(0))),
            Some(NumericBound::exclusive(NumericValue::Integer(1))),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
    assert_eq!(
        NumericRange::new(
            NumericDomain::DoubleFloat,
            Some(NumericBound::inclusive(NumericValue::DoubleFloat(
                f64::NAN.to_bits()
            ))),
            None,
        ),
        Err(NumericRangeError::NaNBoundary)
    );

    let mut arena = TypeArena::new(form(2))?;
    let outer_id = arena.numeric_range(outer)?;
    let inner_id = arena.numeric_range(inner)?;
    let disjoint_id = arena.numeric_range(disjoint)?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let float = named(&mut arena, BuiltinType::Float)?;
    let rational = named(&mut arena, BuiltinType::Rational)?;
    assert_eq!(arena.subtype(inner_id, outer_id)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(disjoint_id, outer_id)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(inner_id, integer)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(inner_id, float)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(integer, inner_id)?, SubtypeRelation::No);
    // Opaque membership cannot prove a numeric relation in HIR's three-valued lattice (docs/src/design/hir.md).
    let opaque_member = arena.member(vec![MemberValue::OpaqueObject {
        owner: CompilationUnitId::new(0),
        index: 0,
    }])?;
    assert_eq!(
        arena.subtype(opaque_member, outer_id)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(inner_id, rational)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn numeric_ranges_use_discrete_integer_and_float_boundaries() -> Result<(), Box<dyn Error>> {
    let integer_middle = NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::exclusive(NumericValue::Integer(0))),
        Some(NumericBound::exclusive(NumericValue::Integer(2))),
    )?;
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(NumericBound::exclusive(NumericValue::Integer(0))),
            Some(NumericBound::exclusive(NumericValue::Integer(1))),
        ),
        Err(NumericRangeError::EmptyInterval)
    );

    let zero = f32::from_bits(0);
    let least_positive = f32::from_bits(1);
    let least_positive_range = NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::exclusive(NumericValue::SingleFloat(
            zero.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            least_positive.to_bits(),
        ))),
    )?;
    assert_eq!(
        NumericRange::new(
            NumericDomain::SingleFloat,
            Some(NumericBound::exclusive(NumericValue::SingleFloat(
                zero.to_bits()
            ))),
            Some(NumericBound::exclusive(NumericValue::SingleFloat(
                least_positive.to_bits(),
            ))),
        ),
        Err(NumericRangeError::EmptyInterval)
    );

    let mut arena = TypeArena::new(form(6))?;
    let integer_singleton = arena.numeric_range(integer_middle)?;
    let integer_member = arena.member(vec![MemberValue::Integer(1)])?;
    let least_positive_id = arena.numeric_range(least_positive_range)?;
    let least_positive_member =
        arena.member(vec![MemberValue::SingleFloat(least_positive.to_bits())])?;
    let zero_range = NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            zero.to_bits(),
        ))),
        Some(NumericBound::exclusive(NumericValue::SingleFloat(
            least_positive.to_bits(),
        ))),
    )?;
    let zero_id = arena.numeric_range(zero_range)?;
    assert_eq!(
        arena.subtype(integer_singleton, integer_member)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(least_positive_id, least_positive_member)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(zero_id, least_positive_id)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(least_positive_id, zero_id)?,
        SubtypeRelation::No
    );

    let one = arena.member(vec![MemberValue::Integer(1)])?;
    let two = arena.member(vec![MemberValue::Integer(2)])?;
    let pair = arena.member(vec![MemberValue::Integer(1), MemberValue::Integer(2)])?;
    let split_union = arena.union([one, two])?;
    assert_eq!(arena.subtype(pair, split_union)?, SubtypeRelation::Yes);

    Ok(())
}

#[test]
fn full_float_ranges_match_their_named_type_and_leave_cross_formats_unknown()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(8))?;
    let single_float = named(&mut arena, BuiltinType::SingleFloat)?;
    let full_single_float =
        arena.numeric_range(NumericRange::new(NumericDomain::SingleFloat, None, None)?)?;
    assert_eq!(
        arena.subtype(single_float, full_single_float)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(full_single_float, single_float)?,
        SubtypeRelation::Yes
    );

    let double_float = named(&mut arena, BuiltinType::DoubleFloat)?;
    let full_double_float =
        arena.numeric_range(NumericRange::new(NumericDomain::DoubleFloat, None, None)?)?;
    assert_eq!(
        arena.subtype(single_float, double_float)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(full_single_float, full_double_float)?,
        SubtypeRelation::Unknown
    );

    let full_integer =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    assert_eq!(
        arena.subtype(full_integer, full_single_float)?,
        SubtypeRelation::No
    );
    Ok(())
}

#[test]
fn complement_relations_do_not_infer_disjointness_from_partial_overlap()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(7))?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let not_number = arena.complement(number)?;
    let mixed_member = arena.member(vec![MemberValue::Integer(1), MemberValue::Nil])?;
    assert_eq!(
        arena.subtype(mixed_member, not_number)?,
        SubtypeRelation::No
    );

    let nil_list = named(&mut arena, BuiltinType::List)?;
    let not_list = arena.complement(nil_list)?;
    let symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 1)?;
    let symbol_member = arena.member(vec![MemberValue::Symbol(symbol)])?;
    assert_eq!(
        arena.subtype(symbol_member, not_list)?,
        SubtypeRelation::Unknown
    );
    let keyword = named(&mut arena, BuiltinType::Keyword)?;
    let not_keyword = arena.complement(keyword)?;
    assert_eq!(
        arena.subtype(symbol_member, not_keyword)?,
        SubtypeRelation::Unknown
    );
    let nil_member = arena.member(vec![MemberValue::Nil])?;
    assert_eq!(
        arena.subtype(symbol_member, nil_member)?,
        SubtypeRelation::Unknown
    );

    let range = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(0))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let not_range = arena.complement(range)?;
    let mixed_range_member =
        arena.member(vec![MemberValue::Integer(1), MemberValue::Integer(9)])?;
    assert_eq!(
        arena.subtype(mixed_range_member, not_range)?,
        SubtypeRelation::No
    );

    let left = arena.member(vec![MemberValue::Integer(1), MemberValue::Integer(2)])?;
    let right = arena.member(vec![MemberValue::Integer(2), MemberValue::Integer(3)])?;
    let excluded = arena.member(vec![MemberValue::Integer(1), MemberValue::Integer(3)])?;
    let intersection = arena.intersection([left, right])?;
    let not_excluded = arena.complement(excluded)?;
    assert_eq!(
        arena.subtype(intersection, not_excluded)?,
        SubtypeRelation::Unknown
    );
    Ok(())
}

#[test]
fn member_types_use_eql_and_reject_foreign_identities() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(3))?;
    let nan = f64::NAN.to_bits();
    let member = arena.member(vec![
        MemberValue::Integer(1),
        MemberValue::Integer(1),
        MemberValue::SingleFloat(0.0_f32.to_bits()),
        MemberValue::SingleFloat((-0.0_f32).to_bits()),
        MemberValue::DoubleFloat(nan),
        MemberValue::DoubleFloat(nan),
    ])?;
    let values = arena.get(member).map(cl_type::ClType::kind);
    assert_eq!(
        values,
        Some(&ClTypeKind::Member(vec![
            MemberValue::Integer(1),
            MemberValue::SingleFloat(0.0_f32.to_bits()),
            MemberValue::DoubleFloat(nan),
            MemberValue::DoubleFloat(nan),
        ]))
    );

    let symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 1)?;
    let other_symbol = SymbolId::try_from_index(CompilationUnitId::new(9), 1)?;
    let nil = arena.member(vec![MemberValue::Nil])?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    assert_eq!(arena.subtype(nil, integer)?, SubtypeRelation::No);
    let symbol_member = arena.member(vec![MemberValue::Symbol(symbol)])?;
    let positive_zero = arena.member(vec![MemberValue::DoubleFloat(0.0_f64.to_bits())])?;
    let negative_zero = arena.member(vec![MemberValue::DoubleFloat((-0.0_f64).to_bits())])?;
    assert_eq!(positive_zero, negative_zero);
    assert!(matches!(
        arena.get(symbol_member).map(cl_type::ClType::kind),
        Some(ClTypeKind::Member(_))
    ));
    assert_eq!(
        arena.member(vec![MemberValue::Symbol(other_symbol)]),
        Err(ClTypeError::ForeignSymbol(other_symbol))
    );
    Ok(())
}

#[test]
fn compound_types_and_opaque_specifiers_retain_structure() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(4))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let real = named(&mut arena, BuiltinType::Real)?;
    let any = arena.any();
    let car_cdr = arena.cons(integer, any)?;
    let broad_cons = arena.cons(any, any)?;
    let symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 2)?;
    let constant = TypeSpecConstId::try_from_index(form(4), 0)?;
    let opaque = arena.opaque(
        symbol,
        [
            OpaqueOperand::Type(integer),
            OpaqueOperand::Type(real),
            OpaqueOperand::Constant(constant),
        ],
    )?;
    let array = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(1),
        simple: true,
    })?;
    let vector = named(&mut arena, BuiltinType::Vector)?;
    let sequence = named(&mut arena, BuiltinType::Sequence)?;
    let function = arena.function(FunctionType {
        parameters: FunctionParameters::Fixed(vec![integer]),
        returns: real,
    })?;
    let function_type = named(&mut arena, BuiltinType::Function)?;
    let multiple_values = arena.multiple_values([integer, real])?;

    assert_eq!(arena.subtype(car_cdr, broad_cons)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(array, vector)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(array, sequence)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(function, function_type)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(multiple_values, arena.any())?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(opaque, integer)?, SubtypeRelation::Unknown);
    assert_eq!(
        arena.get(opaque).map(cl_type::ClType::kind),
        Some(&ClTypeKind::Opaque {
            specifier: symbol,
            operands: vec![
                OpaqueOperand::Type(integer),
                OpaqueOperand::Type(real),
                OpaqueOperand::Constant(constant),
            ],
        })
    );
    Ok(())
}

#[test]
fn invalid_type_and_symbol_owners_are_rejected() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(5))?;
    let foreign_type = TypeId::try_from_index(form(6), 0)?;
    assert_eq!(
        arena.complement(foreign_type),
        Err(ClTypeError::UnknownType(foreign_type))
    );
    let foreign_symbol = SymbolId::try_from_index(CompilationUnitId::new(8), 0)?;
    assert_eq!(
        arena.class(foreign_symbol),
        Err(ClTypeError::ForeignSymbol(foreign_symbol))
    );
    let symbol = SymbolId::try_from_index(CompilationUnitId::new(0), 1)?;
    let foreign_constant = TypeSpecConstId::try_from_index(form(8), 0)?;
    assert_eq!(
        arena.opaque(symbol, [OpaqueOperand::Constant(foreign_constant)]),
        Err(ClTypeError::ForeignConstant(foreign_constant))
    );
    Ok(())
}

use std::error::Error;

use super::{
    ArrayDimensions, ArrayType, BuiltinType, FunctionParameters, FunctionType, MemberValue,
    NumericBound, NumericDomain, NumericRange, NumericValue, OpaqueOperand, SubtypeRelation,
    TypeArena, form, named,
};
use ncl_hir::ids::{CompilationUnitId, SymbolId};

fn member(
    arena: &mut TypeArena,
    values: impl IntoIterator<Item = MemberValue>,
) -> Result<ncl_hir::TypeId, Box<dyn Error>> {
    Ok(arena.member(values.into_iter().collect())?)
}

#[test]
fn standardized_type_names_resolve_to_their_builtin_type_identities() {
    let cases = [
        ("BOOLEAN", BuiltinType::Boolean),
        ("SYMBOL", BuiltinType::Symbol),
        ("KEYWORD", BuiltinType::Keyword),
        ("PACKAGE", BuiltinType::Package),
        ("FUNCTION", BuiltinType::Function),
        ("COMPILED-FUNCTION", BuiltinType::CompiledFunction),
        ("CONS", BuiltinType::Cons),
        ("LIST", BuiltinType::List),
        ("NULL", BuiltinType::Null),
        ("ATOM", BuiltinType::Atom),
        ("NUMBER", BuiltinType::Number),
        ("REAL", BuiltinType::Real),
        ("RATIONAL", BuiltinType::Rational),
        ("INTEGER", BuiltinType::Integer),
        ("FIXNUM", BuiltinType::Fixnum),
        ("BIGNUM", BuiltinType::Bignum),
        ("RATIO", BuiltinType::Ratio),
        ("FLOAT", BuiltinType::Float),
        ("SHORT-FLOAT", BuiltinType::ShortFloat),
        ("SINGLE-FLOAT", BuiltinType::SingleFloat),
        ("DOUBLE-FLOAT", BuiltinType::DoubleFloat),
        ("LONG-FLOAT", BuiltinType::LongFloat),
        ("COMPLEX", BuiltinType::Complex),
        ("CHARACTER", BuiltinType::Character),
        ("BASE-CHAR", BuiltinType::BaseChar),
        ("STANDARD-CHAR", BuiltinType::StandardChar),
        ("EXTENDED-CHAR", BuiltinType::ExtendedChar),
        ("STRING", BuiltinType::String),
        ("BASE-STRING", BuiltinType::BaseString),
        ("SIMPLE-STRING", BuiltinType::SimpleString),
        ("SIMPLE-BASE-STRING", BuiltinType::SimpleBaseString),
        ("BIT-VECTOR", BuiltinType::BitVector),
        ("SIMPLE-BIT-VECTOR", BuiltinType::SimpleBitVector),
        ("VECTOR", BuiltinType::Vector),
        ("SIMPLE-VECTOR", BuiltinType::SimpleVector),
        ("ARRAY", BuiltinType::Array),
        ("SIMPLE-ARRAY", BuiltinType::SimpleArray),
        ("SEQUENCE", BuiltinType::Sequence),
        ("HASH-TABLE", BuiltinType::HashTable),
        ("STREAM", BuiltinType::Stream),
        ("RANDOM-STATE", BuiltinType::RandomState),
        ("RESTART", BuiltinType::Restart),
        ("STRUCTURE-OBJECT", BuiltinType::Structure),
    ];

    // CLHS 4.2.3 specifies the Common Lisp type names represented by this table.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    for (name, expected) in cases {
        assert_eq!(BuiltinType::from_name(name), Some(expected));
    }
    assert_eq!(BuiltinType::from_name("IMPLEMENTATION-TYPE"), None);
}

#[test]
fn named_subtypes_follow_numeric_sequence_and_character_relations() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(103))?;
    let cases = [
        (
            BuiltinType::Fixnum,
            BuiltinType::Integer,
            SubtypeRelation::Yes,
        ),
        (
            BuiltinType::Integer,
            BuiltinType::Rational,
            SubtypeRelation::Yes,
        ),
        (
            BuiltinType::Rational,
            BuiltinType::Real,
            SubtypeRelation::Yes,
        ),
        (BuiltinType::Real, BuiltinType::Number, SubtypeRelation::Yes),
        (
            BuiltinType::String,
            BuiltinType::Sequence,
            SubtypeRelation::Yes,
        ),
        (
            BuiltinType::String,
            BuiltinType::Vector,
            SubtypeRelation::Yes,
        ),
        (
            BuiltinType::SimpleString,
            BuiltinType::String,
            SubtypeRelation::Yes,
        ),
        (
            BuiltinType::StandardChar,
            BuiltinType::Character,
            SubtypeRelation::Yes,
        ),
        (BuiltinType::Null, BuiltinType::List, SubtypeRelation::Yes),
        (
            BuiltinType::Integer,
            BuiltinType::Character,
            SubtypeRelation::No,
        ),
        (
            BuiltinType::Number,
            BuiltinType::Integer,
            SubtypeRelation::No,
        ),
    ];
    // CLHS 4.2.3 lists these standardized type names and type-specifier families.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    for (sub_name, sup_name, expected) in cases {
        let sub = named(&mut arena, sub_name)?;
        let sup = named(&mut arena, sup_name)?;
        assert_eq!(arena.subtype(sub, sup)?, expected);
    }
    Ok(())
}

#[test]
fn finite_members_compare_with_members_numeric_ranges_and_named_types() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(104))?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let null = named(&mut arena, BuiltinType::Null)?;
    let boolean = named(&mut arena, BuiltinType::Boolean)?;
    let symbols = member(
        &mut arena,
        [MemberValue::Symbol(SymbolId::try_from_index(
            CompilationUnitId::new(0),
            0,
        )?)],
    )?;
    let one = member(&mut arena, [MemberValue::Integer(1)])?;
    let one_and_two = member(
        &mut arena,
        [MemberValue::Integer(1), MemberValue::Integer(2)],
    )?;
    let two = member(&mut arena, [MemberValue::Integer(2)])?;
    let nil = member(&mut arena, [MemberValue::Nil])?;
    let nil_and_true = member(&mut arena, [MemberValue::Nil, MemberValue::True])?;
    let opaque = member(
        &mut arena,
        [MemberValue::OpaqueObject {
            owner: CompilationUnitId::new(0),
            index: 4,
        }],
    )?;
    let integer_range = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(0))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let singleton = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
    )?)?;
    let character_type = named(&mut arena, BuiltinType::Character)?;
    let integer_union = arena.union([character_type, integer])?;

    // CLHS 4.2.3 defines MEMBER, EQL values, and numeric range type specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(one, one_and_two)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(one, two)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(one, number)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(one, integer_range)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(one_and_two, integer_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(two, singleton)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(singleton, one)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(opaque, integer_range)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(nil, null)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(nil_and_true, boolean)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(symbols, integer)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(one, integer_union)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn array_function_and_multiple_value_types_preserve_structural_constraints()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(105))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let any = arena.any();
    let rank_two_simple = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(2),
        simple: true,
    })?;
    let axes_two_general = arena.array(ArrayType {
        element_type: Some(number),
        dimensions: ArrayDimensions::Axes(vec![None, None]),
        simple: false,
    })?;
    let axes_two_simple = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Axes(vec![Some(2), Some(3)]),
        simple: true,
    })?;
    let any_shape = arena.array(ArrayType {
        element_type: None,
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;
    let rank_one = arena.array(ArrayType {
        element_type: Some(integer),
        dimensions: ArrayDimensions::Rank(1),
        simple: true,
    })?;
    let int_function = arena.function(FunctionType {
        parameters: FunctionParameters::Fixed(vec![integer]),
        returns: integer,
    })?;
    let number_returning_function = arena.function(FunctionType {
        parameters: FunctionParameters::Fixed(vec![integer]),
        returns: number,
    })?;
    let unknown_parameters = arena.function(FunctionType {
        parameters: FunctionParameters::Any,
        returns: integer,
    })?;
    let values_integer = arena.multiple_values([integer])?;
    let values_number = arena.multiple_values([number])?;
    let values_two = arena.multiple_values([number, number])?;
    let array = named(&mut arena, BuiltinType::Array)?;
    let simple_array = named(&mut arena, BuiltinType::SimpleArray)?;
    let vector = named(&mut arena, BuiltinType::Vector)?;
    let sequence = named(&mut arena, BuiltinType::Sequence)?;
    let function = named(&mut arena, BuiltinType::Function)?;

    // CLHS 4.2.3 specifies compound arrays, functions, and values type specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(
        arena.subtype(rank_two_simple, axes_two_general)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(axes_two_simple, rank_two_simple)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(any_shape, rank_two_simple)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(rank_two_simple, array)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(rank_two_simple, simple_array)?,
        SubtypeRelation::Yes
    );
    assert_eq!(arena.subtype(rank_one, vector)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(rank_one, sequence)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(int_function, number_returning_function)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(int_function, unknown_parameters)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(int_function, function)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(values_integer, values_number)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(values_integer, values_two)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(values_integer, any)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn boolean_cons_complement_and_class_relations_remain_conservative() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(106))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let character = named(&mut arena, BuiltinType::Character)?;
    let number = named(&mut arena, BuiltinType::Number)?;
    let boolean = named(&mut arena, BuiltinType::Boolean)?;
    let either = arena.union([integer, character])?;
    let both = arena.intersection([integer, number])?;
    let not_character = arena.complement(character)?;
    let not_number = arena.complement(number)?;
    let null = named(&mut arena, BuiltinType::Null)?;
    let list = named(&mut arena, BuiltinType::List)?;
    let cons = named(&mut arena, BuiltinType::Cons)?;
    let int_cons = arena.cons(integer, null)?;
    let list_cons = arena.cons(number, list)?;
    let first_class = arena.class(SymbolId::try_from_index(CompilationUnitId::new(0), 1)?)?;
    let same_class = arena.class(SymbolId::try_from_index(CompilationUnitId::new(0), 1)?)?;
    let other_class = arena.class(SymbolId::try_from_index(CompilationUnitId::new(0), 2)?)?;
    let opaque_name = SymbolId::try_from_index(CompilationUnitId::new(0), 3)?;
    let opaque = arena.opaque(opaque_name, [OpaqueOperand::Type(integer)])?;

    // CLHS 4.2.3 defines the source Boolean, CONS, NOT, OR, and AND type forms.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(integer, either)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(either, number)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(both, number)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(integer, not_character)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(integer, not_number)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(boolean, arena.any())?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(int_cons, list_cons)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(int_cons, cons)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(first_class, same_class)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(first_class, other_class)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(opaque, integer)?, SubtypeRelation::Unknown);
    Ok(())
}

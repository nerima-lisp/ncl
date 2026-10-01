#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, FunctionObject, Package, Runtime, ThreadContext, Word, make_cons,
    make_simple_vector, make_specialized_array, make_string, make_symbol,
};
use ncl_types::{
    ArrayDimension, IntegerBound, NamedType, TypeError, TypeSpecifier, Value, parse_type_specifier,
    subtypep, typep,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

#[test]
fn typep_covers_opaque_values_uninterned_symbols_and_sequence_edges() {
    let (runtime, mut ctx) = setup();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(7)]).unwrap();
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Eql(Value::Opaque(vector.bits()))
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Eql(Value::Opaque(vector.bits()))
        )
        .unwrap()
    );
    let uninterned = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap();
    assert!(
        typep(
            &mut ctx,
            uninterned,
            &TypeSpecifier::Named(NamedType::Symbol)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            uninterned,
            &TypeSpecifier::Named(NamedType::Keyword)
        )
        .unwrap()
    );
    assert!(typep(&mut ctx, vector, &TypeSpecifier::Named(NamedType::Sequence)).unwrap());
    assert!(
        typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Named(NamedType::Sequence)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::Sequence)
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::character(65),
            &TypeSpecifier::Named(NamedType::Character)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(65),
            &TypeSpecifier::Named(NamedType::Character)
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_improper_sequences_empty_character_and_keyword_edges() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let keywordp = builtin(&runtime, &mut ctx, "KEYWORDP");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let uninterned = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[uninterned]),
        Ok(Word::NIL)
    );
    let improper = make_cons(&mut ctx, &runtime, Word::character(65), Word::fixnum(9)).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[improper, list_type]),
        Ok(improper)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[improper, string_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let empty = make_string(&mut ctx, &runtime, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[empty, character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let chars = make_string(&mut ctx, &runtime, &['z']).unwrap();
    let character = runtime
        .call_builtin(&mut ctx, coerce, &[chars, character_type])
        .unwrap();
    assert_eq!(character, Word::character(122));
}

#[test]
fn parser_covers_value_conversion_failures_and_dimension_boundaries() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let invalid_member = list(&mut ctx, &runtime, &[integer]);
    let member_form = list(&mut ctx, &runtime, &[member, invalid_member]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, member_form),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let eql_form = list(&mut ctx, &runtime, &[eql, Word::fixnum(11)]);
    assert_eq!(
        parse_type_specifier(&mut ctx, eql_form).unwrap(),
        TypeSpecifier::Eql(Value::Integer(11))
    );
    let rank_form = list(&mut ctx, &runtime, &[array, integer, Word::fixnum(1)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, rank_form).unwrap(),
        TypeSpecifier::Array {
            dimensions: Some(ncl_types::ArrayDimensions::Rank(1)),
            ..
        }
    ));
    let exclusive_dimension = list(&mut ctx, &runtime, &[Word::fixnum(3)]);
    let exclusive_vector = list(
        &mut ctx,
        &runtime,
        &[vector, Word::TRUE, exclusive_dimension],
    );
    assert!(matches!(
        parse_type_specifier(&mut ctx, exclusive_vector),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let invalid_first = list(&mut ctx, &runtime, &[Word::fixnum(1)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_first),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_uncertain_operands_and_all_bound_directions() {
    let named = |name| TypeSpecifier::Named(name);
    assert_eq!(
        subtypep(&named(NamedType::Package), &named(NamedType::Stream)).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Function {
                lambda_list: vec![],
                return_type: Box::new(named(NamedType::T))
            },
            &named(NamedType::Function)
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Exclusive(10)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            }
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Exclusive(10)
            }
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &named(NamedType::Integer),
            &TypeSpecifier::Or(vec![named(NamedType::String), named(NamedType::Integer)])
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::And(vec![named(NamedType::Fixnum), named(NamedType::String)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::And(vec![named(NamedType::Complex), named(NamedType::String)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (false, false)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Satisfies("W17".into()),
            &named(NamedType::T)
        ),
        Err(TypeError::CannotInvoke(_))
    ));
}

#[test]
fn typep_covers_specialized_array_and_vector_size_edges() {
    let (runtime, mut ctx) = setup();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap();
    let chars = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
    let string = make_string(&mut ctx, &runtime, &['A']).unwrap();
    assert!(typep(&mut ctx, bits, &TypeSpecifier::Named(NamedType::BitVector)).unwrap());
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exact(1))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            string,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exclusive(1))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            chars,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
                dimensions: Some(ncl_types::ArrayDimensions::Ranks(vec![ArrayDimension::Any])),
                simple: true
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ncl_types::ArrayDimensions::Wild),
                simple: true
            }
        )
        .unwrap()
    );
}

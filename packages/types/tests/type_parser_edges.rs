#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_specialized_array, make_string,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
    parse_type_specifier, subtypep, typep,
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

fn options() -> ArrayOptions {
    ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::NIL,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    }
}

#[test]
fn typep_exercises_dimension_rank_and_compound_boundaries() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Wild),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: true
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Exact(2),
                    ArrayDimension::Any
                ])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Exact(2),
                    ArrayDimension::Exact(3)
                ])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exclusive(3))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exact(1))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            bits,
            &TypeSpecifier::Named(NamedType::SimpleBitVector)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            string,
            &TypeSpecifier::Named(NamedType::BitVector)
        )
        .unwrap()
    );
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(5), Word::NIL).unwrap();
    assert!(
        typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
                cdr: Box::new(TypeSpecifier::Named(NamedType::Null))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::T)),
                cdr: Box::new(TypeSpecifier::Named(NamedType::T))
            }
        )
        .unwrap()
    );
}

#[test]
fn typep_covers_member_eql_values_and_short_circuit_failures() {
    let (_runtime, mut ctx) = setup();
    assert!(
        typep(
            &mut ctx,
            Word::character(65),
            &TypeSpecifier::Eql(Value::Character(65))
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::character(66),
            &TypeSpecifier::Eql(Value::Character(65))
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::TRUE,
            &TypeSpecifier::Member(vec![Value::Nil, Value::True])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Member(vec![Value::Integer(1), Value::String("NIL".into())])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::String),
                TypeSpecifier::Named(NamedType::Integer)
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::String),
                TypeSpecifier::Named(NamedType::Keyword)
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Named(NamedType::Number)
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Named(NamedType::String)
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::Not(Box::new(TypeSpecifier::Named(NamedType::String)))
        )
        .unwrap()
    );
    assert!(matches!(
        typep(&mut ctx, Word::NIL, &TypeSpecifier::Satisfies("W16".into())),
        Err(TypeError::CannotInvoke(_))
    ));
}

#[test]
fn builtins_cover_vector_array_and_invalid_fallback_coercions() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let array_type = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(4)]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[vector, vector_type]),
        Ok(vector)
    );
    let empty_array = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, array_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, empty_array).unwrap(),
        0
    );
    let bad_range = list(
        &mut ctx,
        &runtime,
        &[integer, Word::fixnum(0), Word::fixnum(1)],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(4), bad_range]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let bad_member = list(&mut ctx, &runtime, &[member, Word::fixnum(1)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(2), bad_member]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let bad_function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, bad_function]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn parser_reaches_domain_error_and_optional_paths() {
    let (runtime, mut ctx) = setup();
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let satisfies = intern(&mut ctx, &runtime, "COMMON-LISP", "SATISFIES");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let p = intern(&mut ctx, &runtime, "COMMON-LISP", "P");
    let not_form = list(&mut ctx, &runtime, &[not, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, not_form).unwrap(),
        TypeSpecifier::Not(_)
    ));
    let eql_form = list(&mut ctx, &runtime, &[eql, Word::fixnum(6)]);
    assert_eq!(
        parse_type_specifier(&mut ctx, eql_form).unwrap(),
        TypeSpecifier::Eql(Value::Integer(6))
    );
    let satisfies_form = list(&mut ctx, &runtime, &[satisfies, p]);
    assert_eq!(
        parse_type_specifier(&mut ctx, satisfies_form).unwrap(),
        TypeSpecifier::Satisfies("P".into())
    );
    for form in [
        list(&mut ctx, &runtime, &[Word::fixnum(1), integer]),
        list(&mut ctx, &runtime, &[eql]),
        list(&mut ctx, &runtime, &[satisfies, Word::fixnum(1)]),
        list(
            &mut ctx,
            &runtime,
            &[integer, Word::TRUE, Word::TRUE, Word::TRUE],
        ),
    ] {
        assert!(matches!(
            parse_type_specifier(&mut ctx, form),
            Err(TypeError::InvalidSpecifier(_))
        ));
    }
}

#[test]
fn subtypep_covers_uncertain_or_and_and_unbounded_ranges() {
    let named = |name| TypeSpecifier::Named(name);
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
            &TypeSpecifier::And(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::String)
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
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Unbounded,
                high: IntegerBound::Unbounded
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            }
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(10)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Unbounded,
                high: IntegerBound::Unbounded
            }
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::T), &named(NamedType::String)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Nil), &named(NamedType::String)).unwrap(),
        (true, true)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Satisfies("W16".into()),
            &named(NamedType::T)
        ),
        Err(TypeError::CannotInvoke(_))
    ));
}

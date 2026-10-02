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
fn builtins_cover_sequence_and_named_fallback_edges() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::character(65)]).unwrap();
    let as_list = runtime
        .call_builtin(&mut ctx, coerce, &[vector, list_type])
        .unwrap();
    assert_eq!(ncl_object::car(&ctx, as_list).unwrap(), Word::character(65));
    let chars = list(
        &mut ctx,
        &runtime,
        &[Word::character(66), Word::character(67)],
    );
    let as_vector = runtime
        .call_builtin(&mut ctx, coerce, &[chars, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, as_vector).unwrap(),
        2
    );
    let as_string = runtime
        .call_builtin(&mut ctx, coerce, &[chars, string_type])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, as_string).unwrap(), 2);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(4), character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, list_type]),
        Ok(Word::NIL)
    );
    let improper = make_cons(&mut ctx, &runtime, Word::character(65), Word::fixnum(1)).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[improper, string_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn typep_covers_array_shapes_ranges_and_values() {
    let (runtime, mut ctx) = setup();
    let matrix = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Character))),
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
                    ArrayDimension::Exact(3),
                    ArrayDimension::Exact(2)
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
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: true
            }
        )
        .unwrap()
    );
    assert!(typep(&mut ctx, bits, &TypeSpecifier::Named(NamedType::BitVector)).unwrap());
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(3),
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(2),
                high: IntegerBound::Inclusive(3)
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(2),
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(2),
                high: IntegerBound::Inclusive(3)
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(9),
            &TypeSpecifier::Eql(Value::Integer(9))
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(9),
            &TypeSpecifier::Eql(Value::Integer(10))
        )
        .unwrap()
    );
}

#[test]
fn parser_covers_nested_domain_forms_and_rejections() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let or = intern(&mut ctx, &runtime, "COMMON-LISP", "OR");
    let and = intern(&mut ctx, &runtime, "COMMON-LISP", "AND");
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let nested_or = list(&mut ctx, &runtime, &[or, integer, Word::NIL]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, nested_or).unwrap(),
        TypeSpecifier::Or(_)
    ));
    let nested_and = list(&mut ctx, &runtime, &[and, integer, Word::TRUE]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, nested_and).unwrap(),
        TypeSpecifier::And(_)
    ));
    let member_form = list(
        &mut ctx,
        &runtime,
        &[member, Word::fixnum(1), Word::character(65)],
    );
    assert!(matches!(
        parse_type_specifier(&mut ctx, member_form).unwrap(),
        TypeSpecifier::Member(_)
    ));
    let not_form = list(&mut ctx, &runtime, &[not, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, not_form).unwrap(),
        TypeSpecifier::Not(_)
    ));
    let invalid_member = list(&mut ctx, &runtime, &[integer]);
    for form in [
        list(&mut ctx, &runtime, &[or, Word::fixnum(1)]),
        list(&mut ctx, &runtime, &[member, invalid_member]),
        list(&mut ctx, &runtime, &[not]),
        list(&mut ctx, &runtime, &[Word::fixnum(1), integer]),
    ] {
        assert!(matches!(
            parse_type_specifier(&mut ctx, form),
            Err(TypeError::InvalidSpecifier(_))
        ));
    }
}

#[test]
fn subtypep_covers_relation_fallbacks_and_range_boundaries() {
    let named = |name| TypeSpecifier::Named(name);
    assert_eq!(
        subtypep(&named(NamedType::Package), &named(NamedType::Stream)).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(&named(NamedType::Cons), &named(NamedType::Null)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(1),
                high: IntegerBound::Inclusive(4)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Exclusive(5)
            }
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Exclusive(0),
                high: IntegerBound::Exclusive(5)
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Inclusive(5)
            }
        )
        .unwrap(),
        (true, true)
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
            &TypeSpecifier::Deftype {
                name: "W18".into(),
                args: vec![]
            },
            &named(NamedType::T)
        ),
        Err(TypeError::UnexpandedDeftype(_))
    ));
}

#[test]
fn typep_covers_specialized_vector_and_cons_negative_edges() {
    let (runtime, mut ctx) = setup();
    let chars = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
    let string = make_string(&mut ctx, &runtime, &['A']).unwrap();
    assert!(
        typep(
            &mut ctx,
            chars,
            &TypeSpecifier::Vector {
                element_type: None,
                size: None
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            chars,
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
                size: Some(ArrayDimension::Exact(2))
            }
        )
        .unwrap()
    );
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert!(
        !typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::String)),
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
    assert!(
        typep(
            &mut ctx,
            Word::TRUE,
            &TypeSpecifier::Named(NamedType::Boolean)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(0),
            &TypeSpecifier::Named(NamedType::Boolean)
        )
        .unwrap()
    );
}

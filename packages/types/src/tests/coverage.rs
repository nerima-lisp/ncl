#![allow(clippy::unwrap_used, reason = "coverage tests assert on type results")]

use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons, make_string};

use crate::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
    parse_type_specifier, serialize_value, subtypep, typep,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, items: &[Word]) -> Word {
    items.iter().rev().copied().fold(Word::NIL, |tail, item| {
        make_cons(ctx, runtime, item, tail).unwrap()
    })
}

#[test]
fn adapter_serializes_every_supported_value() {
    let (runtime, mut ctx) = setup();
    let values = [
        Value::Nil,
        Value::True,
        Value::Integer(-4),
        Value::Character(u32::from('x')),
        Value::String("ok".into()),
        Value::Symbol("INTEGER".into()),
    ];
    for value in values {
        assert!(serialize_value(&mut ctx, &runtime, &value).is_ok());
    }
    assert_eq!(
        serialize_value(&mut ctx, &runtime, &Value::Opaque(4)),
        Err(TypeError::CannotSerialize)
    );
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let parsed = parse_type_specifier(&mut ctx, string);
    assert!(matches!(parsed, Err(TypeError::InvalidSpecifier(_))));
}

#[test]
fn parser_covers_compound_forms_and_rejections() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "INTEGER");
    let star = intern(&mut ctx, &runtime, "*");
    let form = list(&mut ctx, &runtime, &[integer, star, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Unbounded,
            high: IntegerBound::Unbounded,
        }
    );
    let vector = intern(&mut ctx, &runtime, "VECTOR");
    let vector_form = list(&mut ctx, &runtime, &[vector, star, Word::fixnum(2)]);
    let parsed = parse_type_specifier(&mut ctx, vector_form).unwrap();
    assert!(matches!(
        parsed,
        TypeSpecifier::Vector {
            size: Some(ArrayDimension::Exact(2)),
            ..
        }
    ));
    let invalid = list(&mut ctx, &runtime, &[integer, Word::TRUE]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn type_predicates_and_subtype_boundaries_are_asserted() {
    let (_runtime, mut ctx) = setup();
    let five = Word::fixnum(5);
    assert!(typep(&mut ctx, five, &TypeSpecifier::Named(NamedType::Integer)).unwrap());
    assert!(!typep(&mut ctx, five, &TypeSpecifier::Named(NamedType::String)).unwrap());
    assert!(
        typep(
            &mut ctx,
            five,
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(1),
                high: IntegerBound::Exclusive(6)
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            five,
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
            five,
            &TypeSpecifier::Not(Box::new(TypeSpecifier::Named(NamedType::Integer)))
        )
        .unwrap()
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Named(NamedType::Integer),
            &TypeSpecifier::Named(NamedType::Number)
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Satisfies("P".into()),
            &TypeSpecifier::Named(NamedType::T)
        ),
        Err(TypeError::CannotInvoke("P".into()))
    );
    assert_eq!(ArrayDimensions::Wild, ArrayDimensions::Wild);
}

#[test]
fn type_error_display_and_source_are_stable() {
    let errors = [
        TypeError::InvalidForm,
        TypeError::CannotSerialize,
        TypeError::InvalidSpecifier(Word::NIL),
        TypeError::CannotInvoke("P".into()),
        TypeError::UnexpandedDeftype("X".into()),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
    let object = TypeError::from(ncl_object::ObjectError::TypeError);
    assert!(std::error::Error::source(&object).is_some());
}

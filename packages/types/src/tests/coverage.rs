#![allow(clippy::unwrap_used, reason = "coverage tests assert on type results")]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_cons, make_double, make_ratio, make_simple_vector,
    make_specialized_array, make_string,
};

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

fn setup_registered() -> (Runtime, ThreadContext) {
    let (runtime, ctx) = setup();
    crate::register(&runtime).unwrap();
    crate::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn array_options() -> ArrayOptions {
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

#[test]
fn builtins_assert_coercions_and_type_names() {
    let (runtime, mut ctx) = setup_registered();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let float = intern(&mut ctx, &runtime, "DOUBLE-FLOAT");
    let function = intern(&mut ctx, &runtime, "FUNCTION");
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, -((1_i128 << 40) + 3))
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let bignum_float = runtime
        .call_builtin(&mut ctx, coerce, &[bignum, float])
        .unwrap();
    let ratio_float = runtime
        .call_builtin(&mut ctx, coerce, &[ratio, float])
        .unwrap();
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(bignum_float)).unwrap(),
        -(1099511627776.0 + 3.0)
    );
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(ratio_float)).unwrap(),
        1.5
    );
    let type_of_symbol = intern(&mut ctx, &runtime, "TYPE-OF");
    let function_word = runtime
        .call_builtin(&mut ctx, coerce, &[type_of_symbol, function])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, function_word),
        ncl_object::ObjectRef::Function(_)
    ));
    let direct_function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[direct_function, function])
            .unwrap(),
        direct_function
    );
    let double = make_double(&mut ctx, &runtime, 4.25).unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[double.into(), float])
            .unwrap(),
        double.into()
    );
    let fixnum = intern(&mut ctx, &runtime, "FIXNUM");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, type_of, &[Word::fixnum(4)])
            .unwrap(),
        fixnum
    );
}

#[test]
fn typep_asserts_numeric_array_and_cons_boundaries() {
    let (runtime, mut ctx) = setup_registered();
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(3))
        .unwrap()
        .into();
    let float: Word = make_double(&mut ctx, &runtime, -3.5).unwrap().into();
    for (object, named) in [
        (bignum, NamedType::Number),
        (bignum, NamedType::Real),
        (ratio, NamedType::Rational),
        (float, NamedType::Float),
    ] {
        assert!(typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap());
    }
    for named in [
        NamedType::Nil,
        NamedType::ShortFloat,
        NamedType::LongFloat,
        NamedType::RandomState,
        NamedType::Restart,
        NamedType::ExtendedChar,
    ] {
        assert!(!typep(&mut ctx, Word::TRUE, &TypeSpecifier::Named(named)).unwrap());
    }
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 2], array_options()).unwrap();
    assert!(typep(&mut ctx, vector, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(typep(&mut ctx, matrix, &TypeSpecifier::Named(NamedType::Array)).unwrap());
    assert!(!typep(&mut ctx, matrix, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
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
            bits,
            &TypeSpecifier::Named(NamedType::SimpleBitVector)
        )
        .unwrap()
    );
    let improper = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert!(
        !typep(
            &mut ctx,
            improper,
            &TypeSpecifier::Cons {
                car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
                cdr: Box::new(TypeSpecifier::Named(NamedType::Null)),
            }
        )
        .unwrap()
    );
}

#[test]
fn parser_and_subtype_assert_boundary_values() {
    let (runtime, mut ctx) = setup_registered();
    let integer = intern(&mut ctx, &runtime, "INTEGER");
    let low = list(&mut ctx, &runtime, &[Word::fixnum(2)]);
    let range = list(&mut ctx, &runtime, &[integer, low, Word::fixnum(9)]);
    assert_eq!(
        parse_type_specifier(&mut ctx, range).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Exclusive(2),
            high: IntegerBound::Inclusive(9),
        }
    );
    let simple_array = intern(&mut ctx, &runtime, "SIMPLE-ARRAY");
    let dims = list(&mut ctx, &runtime, &[Word::fixnum(2), Word::fixnum(3)]);
    let form = list(&mut ctx, &runtime, &[simple_array, integer, dims]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
            dimensions: Some(ArrayDimensions::Ranks(vec![
                ArrayDimension::Exact(2),
                ArrayDimension::Exact(3)
            ])),
            simple: true,
        }
    );
    let named = |value| TypeSpecifier::Named(value);
    assert_eq!(
        subtypep(&named(NamedType::Fixnum), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Cons), &named(NamedType::Number)).unwrap(),
        (false, true)
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
}

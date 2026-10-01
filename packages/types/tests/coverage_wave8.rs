#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_string,
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
fn typep_covers_numeric_families_and_object_shape_negatives() {
    let (runtime, mut ctx) = setup();
    let big: Word = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(2))
        .unwrap()
        .into();
    let float: Word = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(1))
        .unwrap()
        .into();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let array = make_array(&mut ctx, &runtime, &[1, 2], options()).unwrap();
    let cases = [
        (big, NamedType::Integer),
        (big, NamedType::Real),
        (big, NamedType::Number),
        (ratio, NamedType::Rational),
        (ratio, NamedType::Real),
        (float, NamedType::Float),
        (float, NamedType::Real),
        (complex, NamedType::Number),
        (string, NamedType::Sequence),
        (vector, NamedType::Array),
        (bits, NamedType::BitVector),
        (array, NamedType::Array),
    ];
    for (object, named) in cases {
        assert!(
            typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap(),
            "{named:?}"
        );
    }
    for (object, named) in [
        (big, NamedType::Fixnum),
        (ratio, NamedType::Integer),
        (float, NamedType::Rational),
        (complex, NamedType::Real),
        (array, NamedType::Vector),
        (vector, NamedType::HashTable),
        (Word::NIL, NamedType::Structure),
    ] {
        assert!(
            !typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap(),
            "{named:?}"
        );
    }
}

#[test]
fn typep_covers_cons_dimensions_and_value_comparisons() {
    let (runtime, mut ctx) = setup();
    let good = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let bad_tail = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::TRUE).unwrap();
    let spec = TypeSpecifier::Cons {
        car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
        cdr: Box::new(TypeSpecifier::Named(NamedType::Null)),
    };
    assert!(typep(&mut ctx, good, &spec).unwrap());
    assert!(!typep(&mut ctx, bad_tail, &spec).unwrap());
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::String))),
                size: Some(ArrayDimension::Exclusive(3)),
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
                size: Some(ArrayDimension::Exclusive(2)),
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(9),
            &TypeSpecifier::Member(vec![Value::Integer(3), Value::Integer(9)])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(9),
            &TypeSpecifier::Eql(Value::Integer(8))
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_sequence_numeric_and_function_coercions() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let float_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let chars = list(
        &mut ctx,
        &runtime,
        &[Word::character(120), Word::character(121)],
    );
    let vector = runtime
        .call_builtin(&mut ctx, coerce, &[chars, vector_type])
        .unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, vector).unwrap(), 2);
    let list = runtime
        .call_builtin(&mut ctx, coerce, &[vector, list_type])
        .unwrap();
    assert!(list.is_cons());
    let string = runtime
        .call_builtin(&mut ctx, coerce, &[list, string_type])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, string).unwrap(), 2);
    for value in [Word::fixnum(-12), Word::fixnum(12)] {
        let converted = runtime
            .call_builtin(&mut ctx, coerce, &[value, float_type])
            .unwrap();
        assert!(matches!(
            ncl_object::classify_object(&ctx, converted),
            ncl_object::ObjectRef::DoubleFloat(_)
        ));
    }
    let function_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "TYPE-OF");
    let function = runtime
        .call_builtin(&mut ctx, coerce, &[function_symbol, function_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, function),
        ncl_object::ObjectRef::Function(_) | ncl_object::ObjectRef::Closure(_)
    ));
}

#[test]
fn parser_covers_integer_exclusive_and_invalid_single_forms() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let low = list(&mut ctx, &runtime, &[Word::fixnum(-2)]);
    let high = list(&mut ctx, &runtime, &[Word::fixnum(4)]);
    let range = list(&mut ctx, &runtime, &[integer, low, high]);
    assert_eq!(
        parse_type_specifier(&mut ctx, range).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Exclusive(-2),
            high: IntegerBound::Exclusive(4),
        }
    );
    let or = intern(&mut ctx, &runtime, "COMMON-LISP", "OR");
    let or_form = list(&mut ctx, &runtime, &[or, integer, Word::TRUE]);
    assert_eq!(
        parse_type_specifier(&mut ctx, or_form).unwrap(),
        TypeSpecifier::Or(vec![
            TypeSpecifier::Named(NamedType::Integer),
            TypeSpecifier::Named(NamedType::T),
        ])
    );
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let invalid_not = list(&mut ctx, &runtime, &[not]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_not),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let invalid_member = list(&mut ctx, &runtime, &[member, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_member),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_sequence_array_function_and_range_boundaries() {
    let named = |name| TypeSpecifier::Named(name);
    for (sub, sup) in [
        (NamedType::Cons, NamedType::Sequence),
        (NamedType::String, NamedType::Array),
        (NamedType::SimpleVector, NamedType::Vector),
        (NamedType::BitVector, NamedType::Sequence),
        (NamedType::CompiledFunction, NamedType::Function),
        (NamedType::Keyword, NamedType::Symbol),
    ] {
        assert_eq!(subtypep(&named(sub), &named(sup)).unwrap(), (true, true));
    }
    for (sub, sup) in [
        (NamedType::Ratio, NamedType::Fixnum),
        (NamedType::Complex, NamedType::Real),
        (NamedType::Cons, NamedType::Null),
        (NamedType::Number, NamedType::Symbol),
    ] {
        assert_eq!(subtypep(&named(sub), &named(sup)).unwrap(), (false, true));
    }
    let ranges = |low, high| TypeSpecifier::IntegerRange { low, high };
    assert_eq!(
        subtypep(
            &ranges(IntegerBound::Exclusive(1), IntegerBound::Exclusive(4)),
            &ranges(IntegerBound::Inclusive(1), IntegerBound::Inclusive(4)),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &ranges(IntegerBound::Inclusive(1), IntegerBound::Inclusive(4)),
            &ranges(IntegerBound::Exclusive(1), IntegerBound::Exclusive(4)),
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::And(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::String),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &named(NamedType::Boolean),
            &TypeSpecifier::Or(vec![named(NamedType::String)]),
        )
        .unwrap(),
        (false, false)
    );
}

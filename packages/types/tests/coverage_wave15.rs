#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_code_object, make_cons, make_double, make_instance,
    make_ratio, make_readtable, make_simple_vector, make_specialized_array, make_string,
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
fn builtins_cover_each_sequence_source_and_numeric_coercion() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let float_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::character(65), Word::character(66)],
    )
    .unwrap();
    let string = make_string(&mut ctx, &runtime, &['c', 'd']).unwrap();
    let chars = list(
        &mut ctx,
        &runtime,
        &[Word::character(101), Word::character(102)],
    );
    let as_list = runtime
        .call_builtin(&mut ctx, coerce, &[vector, list_type])
        .unwrap();
    assert_eq!(ncl_object::car(&ctx, as_list).unwrap(), Word::character(65));
    let string_list = runtime
        .call_builtin(&mut ctx, coerce, &[string, list_type])
        .unwrap();
    assert_eq!(
        ncl_object::car(&ctx, string_list).unwrap(),
        Word::character(99)
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
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 68)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(2))
        .unwrap()
        .into();
    let double: Word = make_double(&mut ctx, &runtime, -1.25).unwrap().into();
    for number in [Word::fixnum(-9), bignum, ratio, double] {
        let result = runtime
            .call_builtin(&mut ctx, coerce, &[number, float_type])
            .unwrap();
        assert!(matches!(
            ncl_object::classify_object(&ctx, result),
            ncl_object::ObjectRef::DoubleFloat(_)
        ));
    }
    let function_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "TYPE-OF");
    let function = runtime
        .call_builtin(&mut ctx, coerce, &[function_symbol, function_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, function),
        ncl_object::ObjectRef::Function(_)
    ));
    let empty_string = runtime
        .call_builtin(&mut ctx, coerce, &[Word::NIL, string_type])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, empty_string).unwrap(), 0);
}

#[test]
fn typep_covers_cons_member_arrays_and_short_circuit_results() {
    let (runtime, mut ctx) = setup();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(8), Word::NIL).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap();
    let cons_spec = TypeSpecifier::Cons {
        car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
        cdr: Box::new(TypeSpecifier::Named(NamedType::Null)),
    };
    assert!(typep(&mut ctx, cons, &cons_spec).unwrap());
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
        typep(
            &mut ctx,
            Word::fixnum(8),
            &TypeSpecifier::Member(vec![Value::Integer(7), Value::Integer(8)])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(8),
            &TypeSpecifier::Member(vec![Value::Integer(7), Value::Integer(9)])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(8),
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Satisfies("never".into())
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(8),
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::String),
                TypeSpecifier::Satisfies("never".into())
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(2)),
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
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exact(2)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(typep(&mut ctx, bits, &TypeSpecifier::Named(NamedType::BitVector)).unwrap());
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::BitVector)
        )
        .unwrap()
    );
    assert!(matches!(
        typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Deftype {
                name: "W15".into(),
                args: vec![]
            }
        ),
        Err(TypeError::UnexpandedDeftype(_))
    ));
}

#[test]
fn parser_covers_every_operator_empty_defaults_and_invalid_forms() {
    let (runtime, mut ctx) = setup();
    let names = [
        ("OR", TypeSpecifier::Or(vec![])),
        ("AND", TypeSpecifier::And(vec![])),
        ("VALUES", TypeSpecifier::Values(vec![])),
    ];
    for (name, expected) in names {
        let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", name);
        let form = list(&mut ctx, &runtime, &[symbol]);
        assert_eq!(parse_type_specifier(&mut ctx, form).unwrap(), expected);
    }
    let cons = intern(&mut ctx, &runtime, "COMMON-LISP", "CONS");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let cons_form = list(&mut ctx, &runtime, &[cons]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, cons_form).unwrap(),
        TypeSpecifier::Cons { .. }
    ));
    let array_form = list(&mut ctx, &runtime, &[array, integer, star]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array { .. }
    ));
    let vector_form = list(&mut ctx, &runtime, &[vector, star, Word::fixnum(3)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, vector_form).unwrap(),
        TypeSpecifier::Vector {
            size: Some(ArrayDimension::Exact(3)),
            ..
        }
    ));
    let function_form = list(&mut ctx, &runtime, &[function, star]);
    assert!(
        matches!(parse_type_specifier(&mut ctx, function_form).unwrap(), TypeSpecifier::Function { lambda_list, .. } if lambda_list.is_empty())
    );
    let bad_non_symbol = list(&mut ctx, &runtime, &[Word::fixnum(1), integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, bad_non_symbol),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let bad_integer = list(&mut ctx, &runtime, &[integer, Word::TRUE]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, bad_integer),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let bad_vector = list(
        &mut ctx,
        &runtime,
        &[vector, Word::TRUE, Word::TRUE, Word::TRUE],
    );
    assert!(matches!(
        parse_type_specifier(&mut ctx, bad_vector),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_fallback_ranges_and_composite_shortcuts() {
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
                low: IntegerBound::Unbounded,
                high: IntegerBound::Inclusive(3)
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
        subtypep(
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(0),
                high: IntegerBound::Unbounded
            },
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Inclusive(1),
                high: IntegerBound::Unbounded
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
            &named(NamedType::String),
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::String)])
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &named(NamedType::Fixnum),
            &TypeSpecifier::And(vec![named(NamedType::Integer), named(NamedType::Number)])
        )
        .unwrap(),
        (false, false)
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
            &TypeSpecifier::And(vec![named(NamedType::String), named(NamedType::Complex)]),
            &named(NamedType::Integer)
        )
        .unwrap(),
        (false, false)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Deftype {
                name: "W15".into(),
                args: vec![]
            },
            &named(NamedType::T)
        ),
        Err(TypeError::UnexpandedDeftype(_))
    ));
}

#[test]
fn parser_named_type_dispatch_accepts_remaining_names() {
    let (runtime, mut ctx) = setup();
    let names = [
        ("KEYWORD", NamedType::Keyword),
        ("CONS", NamedType::Cons),
        ("LIST", NamedType::List),
        ("NULL", NamedType::Null),
        ("ATOM", NamedType::Atom),
        ("REAL", NamedType::Real),
        ("RATIONAL", NamedType::Rational),
        ("FIXNUM", NamedType::Fixnum),
        ("DOUBLE-FLOAT", NamedType::DoubleFloat),
        ("SIMPLE-STRING", NamedType::SimpleString),
        ("BIT-VECTOR", NamedType::BitVector),
        ("SIMPLE-VECTOR", NamedType::SimpleVector),
        ("SEQUENCE", NamedType::Sequence),
        ("HASH-TABLE", NamedType::HashTable),
        ("STREAM", NamedType::Stream),
        ("STRUCTURE-OBJECT", NamedType::Structure),
    ];
    for (name, expected) in names {
        let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", name);
        assert_eq!(
            parse_type_specifier(&mut ctx, symbol).unwrap(),
            TypeSpecifier::Named(expected)
        );
    }
}

#[test]
fn builtins_classify_closure_instance_and_readtable_objects() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let name = intern(&mut ctx, &runtime, "COMMON-LISP", "W15-CLOSURE");
    let vector = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let bignum = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 65).unwrap();
    let code = make_code_object(&mut ctx, &runtime, 1, 0, bignum.into(), name, vector).unwrap();
    let closure =
        ncl_object::make_closure(&mut ctx, &runtime, 1, name, Word::NIL, code, &[]).unwrap();
    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[]).unwrap();
    let readtable = make_readtable(&mut ctx, &runtime, Word::NIL, Word::NIL, Word::NIL).unwrap();
    for (object, expected) in [
        (closure.into(), "FUNCTION"),
        (instance.into(), "STANDARD-OBJECT"),
        (readtable.into(), "READTABLE"),
    ] {
        let actual = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        let expected = intern(&mut ctx, &runtime, "COMMON-LISP", expected);
        assert_eq!(actual, expected);
    }
}

#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_stream, make_structure,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, NamedType, TypeError, TypeSpecifier, parse_type_specifier,
    typep,
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
fn typep_reaches_object_kinds_and_character_boundaries() {
    let (runtime, mut ctx) = setup();
    let package = runtime.find_package(&mut ctx, "COMMON-LISP").unwrap();
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let float: Word = make_double(&mut ctx, &runtime, 1.25).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let stream: Word = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap()
    .into();
    let layout = runtime.register_structure_layout(0).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[]).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    let values = [
        (package, NamedType::Package),
        (bignum, NamedType::Bignum),
        (ratio, NamedType::Ratio),
        (float, NamedType::DoubleFloat),
        (complex, NamedType::Complex),
        (stream, NamedType::Stream),
        (structure, NamedType::Structure),
        (table, NamedType::HashTable),
        (function, NamedType::CompiledFunction),
    ];
    for (object, named) in values {
        assert!(typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap());
    }
    assert!(
        typep(
            &mut ctx,
            Word::character(10),
            &TypeSpecifier::Named(NamedType::StandardChar)
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::character(126),
            &TypeSpecifier::Named(NamedType::StandardChar)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::character(127),
            &TypeSpecifier::Named(NamedType::StandardChar)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::character(10),
            &TypeSpecifier::Named(NamedType::ExtendedChar)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::Bignum)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::Ratio)
        )
        .unwrap()
    );
}

#[test]
fn typep_covers_exclusive_dimensions_and_empty_compound_results() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let array = make_array(&mut ctx, &runtime, &[2], options()).unwrap();
    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
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
                size: Some(ArrayDimension::Exclusive(2))
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            array,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exclusive(3)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            array,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exclusive(2)])),
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            specialized,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Wild),
                simple: true
            }
        )
        .unwrap()
    );
    assert!(!typep(&mut ctx, Word::fixnum(1), &TypeSpecifier::Or(vec![])).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(1), &TypeSpecifier::And(vec![])).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(1), &TypeSpecifier::Values(vec![])).unwrap());
}

#[test]
fn builtins_cover_vector_sequence_numeric_and_function_coercions() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let float_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(3), Word::fixnum(4)]).unwrap();
    let as_list = runtime
        .call_builtin(&mut ctx, coerce, &[vector, list_type])
        .unwrap();
    assert!(as_list.is_cons());
    assert_eq!(ncl_object::car(&ctx, as_list).unwrap(), Word::fixnum(3));
    let values = list(
        &mut ctx,
        &runtime,
        &[Word::character(65), Word::character(66)],
    );
    let as_vector = runtime
        .call_builtin(&mut ctx, coerce, &[values, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, as_vector).unwrap(),
        2
    );
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 65))
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(2))
        .unwrap()
        .into();
    for number in [Word::fixnum(-7), bignum, ratio] {
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
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn parser_covers_all_named_dispatches_and_domain_forms() {
    let (runtime, mut ctx) = setup();
    let names = [
        ("T", NamedType::T),
        ("NIL", NamedType::Nil),
        ("BOOLEAN", NamedType::Boolean),
        ("PACKAGE", NamedType::Package),
        ("COMPILED-FUNCTION", NamedType::CompiledFunction),
        ("BIGNUM", NamedType::Bignum),
        ("RATIO", NamedType::Ratio),
        ("SHORT-FLOAT", NamedType::ShortFloat),
        ("SINGLE-FLOAT", NamedType::SingleFloat),
        ("LONG-FLOAT", NamedType::LongFloat),
        ("CHARACTER", NamedType::Character),
        ("BASE-CHAR", NamedType::BaseChar),
        ("STANDARD-CHAR", NamedType::StandardChar),
        ("EXTENDED-CHAR", NamedType::ExtendedChar),
        ("BASE-STRING", NamedType::BaseString),
        ("SIMPLE-BASE-STRING", NamedType::SimpleBaseString),
        ("RANDOM-STATE", NamedType::RandomState),
        ("RESTART", NamedType::Restart),
        ("STRUCTURE-OBJECT", NamedType::Structure),
        ("VALUES", NamedType::ValuesType),
    ];
    for (name, expected) in names {
        let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", name);
        assert_eq!(
            parse_type_specifier(&mut ctx, symbol).unwrap(),
            TypeSpecifier::Named(expected)
        );
    }
    let or = intern(&mut ctx, &runtime, "COMMON-LISP", "OR");
    let and = intern(&mut ctx, &runtime, "COMMON-LISP", "AND");
    let values = intern(&mut ctx, &runtime, "COMMON-LISP", "VALUES");
    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let satisfies = intern(&mut ctx, &runtime, "COMMON-LISP", "SATISFIES");
    let predicate = intern(&mut ctx, &runtime, "COMMON-LISP", "P");
    let cons = intern(&mut ctx, &runtime, "COMMON-LISP", "CONS");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    for form in [
        list(&mut ctx, &runtime, &[or, integer]),
        list(&mut ctx, &runtime, &[and, integer]),
        list(&mut ctx, &runtime, &[values, integer]),
        list(&mut ctx, &runtime, &[member, Word::fixnum(1)]),
        list(&mut ctx, &runtime, &[eql, Word::fixnum(1)]),
        list(&mut ctx, &runtime, &[satisfies, predicate]),
        list(&mut ctx, &runtime, &[cons, integer, Word::NIL]),
    ] {
        assert!(parse_type_specifier(&mut ctx, form).is_ok());
    }
    let malformed = list(&mut ctx, &runtime, &[eql, Word::fixnum(1), Word::fixnum(2)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

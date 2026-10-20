#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_string,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier,
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
fn typep_covers_false_object_kinds_and_all_numeric_families() {
    let (runtime, mut ctx) = setup();
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 72)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(9), Word::fixnum(4))
        .unwrap()
        .into();
    let float: Word = make_double(&mut ctx, &runtime, 4.5).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .into();
    let string = make_string(&mut ctx, &runtime, &['q']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let package = runtime.find_package(&mut ctx, "COMMON-LISP").unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    for (object, named) in [
        (bignum, NamedType::Integer),
        (bignum, NamedType::Bignum),
        (ratio, NamedType::Ratio),
        (float, NamedType::Float),
        (float, NamedType::DoubleFloat),
        (complex, NamedType::Complex),
        (string, NamedType::SimpleString),
        (vector, NamedType::SimpleVector),
        (package, NamedType::Package),
        (table, NamedType::HashTable),
    ] {
        assert!(typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap());
    }
    for (object, named) in [
        (Word::fixnum(1), NamedType::Package),
        (Word::NIL, NamedType::CompiledFunction),
        (Word::fixnum(1), NamedType::Ratio),
        (Word::fixnum(1), NamedType::Complex),
        (Word::NIL, NamedType::HashTable),
        (Word::fixnum(1), NamedType::SimpleVector),
        (Word::NIL, NamedType::Stream),
        (Word::NIL, NamedType::Structure),
    ] {
        assert!(!typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap());
    }
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(2),
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Unbounded,
                high: IntegerBound::Unbounded
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            bignum,
            &TypeSpecifier::IntegerRange {
                low: IntegerBound::Unbounded,
                high: IntegerBound::Unbounded
            }
        )
        .unwrap()
    );
}

#[test]
fn typep_covers_vector_array_and_compound_negative_paths() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let character_array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65), Word::character(66)],
    )
    .unwrap();
    assert!(
        typep(
            &mut ctx,
            matrix,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Character))),
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Any,
                    ArrayDimension::Exact(3)
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
                    ArrayDimension::Exact(4)
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
                simple: false
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            character_array,
            &TypeSpecifier::Vector {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
                size: Some(ArrayDimension::Exact(2))
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
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
            Word::fixnum(4),
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Named(NamedType::String)
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(4),
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
            Word::fixnum(4),
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
            Word::fixnum(4),
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Named(NamedType::String)
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Eql(ncl_types::Value::String("different".into()))
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_keyword_type_names_specialized_arrays_and_bad_specs() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let keywordp = builtin(&runtime, &mut ctx, "KEYWORDP");
    let subtypep_builtin = builtin(&runtime, &mut ctx, "SUBTYPEP");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "W14");
    let ordinary = intern(&mut ctx, &runtime, "COMMON-LISP", "W14");
    let character_array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
    let bit_array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[keyword]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[ordinary]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[Word::fixnum(1)]),
        Ok(Word::NIL)
    );
    let char_name = runtime
        .call_builtin(&mut ctx, type_of, &[character_array])
        .unwrap();
    let bit_name = runtime
        .call_builtin(&mut ctx, type_of, &[bit_array])
        .unwrap();
    let simple_array_name = intern(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-ARRAY");
    let simple_bit_vector_name = intern(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-BIT-VECTOR");
    assert_eq!(
        ncl_object::symbol_name(&ctx, char_name).unwrap(),
        ncl_object::symbol_name(&ctx, simple_array_name).unwrap()
    );
    assert_eq!(
        ncl_object::symbol_name(&ctx, bit_name).unwrap(),
        ncl_object::symbol_name(&ctx, simple_bit_vector_name).unwrap()
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            subtypep_builtin,
            &[Word::fixnum(1), Word::fixnum(2)]
        ),
        Ok(Word::NIL)
    );
    assert_eq!(ctx.values(), &[Word::NIL]);
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let bad = list(&mut ctx, &runtime, &[integer, Word::TRUE, Word::TRUE]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), bad]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn subtypep_exercises_all_range_containment_and_named_disjointness() {
    let named = |name| TypeSpecifier::Named(name);
    let cases = [
        (
            IntegerBound::Unbounded,
            IntegerBound::Unbounded,
            IntegerBound::Unbounded,
            IntegerBound::Unbounded,
            (true, true),
        ),
        (
            IntegerBound::Unbounded,
            IntegerBound::Inclusive(4),
            IntegerBound::Inclusive(0),
            IntegerBound::Inclusive(4),
            (false, false),
        ),
        (
            IntegerBound::Inclusive(1),
            IntegerBound::Unbounded,
            IntegerBound::Inclusive(0),
            IntegerBound::Inclusive(5),
            (false, false),
        ),
        (
            IntegerBound::Inclusive(1),
            IntegerBound::Inclusive(4),
            IntegerBound::Exclusive(0),
            IntegerBound::Exclusive(5),
            (true, true),
        ),
        (
            IntegerBound::Exclusive(1),
            IntegerBound::Exclusive(4),
            IntegerBound::Inclusive(1),
            IntegerBound::Inclusive(4),
            (true, true),
        ),
    ];
    for (low, high, sup_low, sup_high, expected) in cases {
        assert_eq!(
            subtypep(
                &TypeSpecifier::IntegerRange { low, high },
                &TypeSpecifier::IntegerRange {
                    low: sup_low,
                    high: sup_high
                }
            )
            .unwrap(),
            expected
        );
    }
    for (sub, sup) in [
        (NamedType::Fixnum, NamedType::Integer),
        (NamedType::Integer, NamedType::Rational),
        (NamedType::Rational, NamedType::Real),
        (NamedType::Real, NamedType::Number),
        (NamedType::String, NamedType::Array),
        (NamedType::Vector, NamedType::Sequence),
        (NamedType::List, NamedType::Sequence),
        (NamedType::SimpleBitVector, NamedType::BitVector),
    ] {
        assert_eq!(subtypep(&named(sub), &named(sup)).unwrap(), (true, true));
    }
    for (sub, sup) in [
        (NamedType::Integer, NamedType::Float),
        (NamedType::Ratio, NamedType::Fixnum),
        (NamedType::Cons, NamedType::Null),
        (NamedType::Number, NamedType::Symbol),
        (NamedType::Symbol, NamedType::Cons),
    ] {
        assert_eq!(subtypep(&named(sub), &named(sup)).unwrap(), (false, true));
    }
    assert_eq!(
        subtypep(&named(NamedType::T), &named(NamedType::Nil)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Nil), &named(NamedType::String)).unwrap(),
        (true, true)
    );
}

#[test]
fn parser_covers_empty_optional_forms_and_domain_errors() {
    let (runtime, mut ctx) = setup();
    let values = intern(&mut ctx, &runtime, "COMMON-LISP", "VALUES");
    let cons = intern(&mut ctx, &runtime, "COMMON-LISP", "CONS");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let simple_array = intern(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let values_form = list(&mut ctx, &runtime, &[values]);
    let cons_form = list(&mut ctx, &runtime, &[cons]);
    let array_form = list(&mut ctx, &runtime, &[array]);
    let simple_array_form = list(&mut ctx, &runtime, &[simple_array]);
    let vector_form = list(&mut ctx, &runtime, &[vector]);
    assert_eq!(
        parse_type_specifier(&mut ctx, values_form).unwrap(),
        TypeSpecifier::Values(vec![])
    );
    assert_eq!(
        parse_type_specifier(&mut ctx, cons_form).unwrap(),
        TypeSpecifier::Cons {
            car: Box::new(TypeSpecifier::Named(NamedType::T)),
            cdr: Box::new(TypeSpecifier::Named(NamedType::T))
        }
    );
    assert_eq!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array {
            element_type: None,
            dimensions: None,
            simple: false
        }
    );
    assert_eq!(
        parse_type_specifier(&mut ctx, simple_array_form).unwrap(),
        TypeSpecifier::Array {
            element_type: None,
            dimensions: None,
            simple: true
        }
    );
    assert_eq!(
        parse_type_specifier(&mut ctx, vector_form).unwrap(),
        TypeSpecifier::Vector {
            element_type: None,
            size: None
        }
    );
    let function_form = list(&mut ctx, &runtime, &[function, star]);
    assert!(
        matches!(parse_type_specifier(&mut ctx, function_form).unwrap(), TypeSpecifier::Function { lambda_list, .. } if lambda_list.is_empty())
    );
    for form in [
        list(&mut ctx, &runtime, &[array, Word::fixnum(1), Word::TRUE]),
        list(&mut ctx, &runtime, &[vector, Word::TRUE, Word::fixnum(-1)]),
        list(&mut ctx, &runtime, &[function, Word::TRUE]),
        list(
            &mut ctx,
            &runtime,
            &[cons, Word::TRUE, Word::TRUE, Word::TRUE],
        ),
    ] {
        assert!(matches!(
            parse_type_specifier(&mut ctx, form),
            Err(TypeError::InvalidSpecifier(_))
        ));
    }
}

#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_specialized_array,
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

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> String {
    let name = ncl_object::symbol_name(ctx, symbol).unwrap();
    (0..ncl_object::string_length(ctx, name).unwrap())
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
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
fn builtins_cover_type_names_and_coerce_failures() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let keywordp = builtin(&runtime, &mut ctx, "KEYWORDP");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let ordinary = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE5-SYMBOL");
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "WAVE5-KEYWORD");
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();

    for (object, expected) in [
        (ordinary, "SYMBOL"),
        (keyword, "KEYWORD"),
        (package, "PACKAGE"),
        (specialized, "SIMPLE-ARRAY"),
        (table, "HASH-TABLE"),
    ] {
        let actual = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        assert_eq!(symbol_text(&ctx, actual), expected);
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[ordinary]),
        Ok(Word::NIL)
    );

    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let float_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let invalid_spec = Word::fixnum(99);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), invalid_spec]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let bad_chars = list(&mut ctx, &runtime, &[Word::fixnum(1)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[bad_chars, string_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let improper = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[improper, string_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), vector_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::character(65), float_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn function_designators_and_composite_coercions_are_checked() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let type_of_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "TYPE-OF");
    let unbound = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE5-UNBOUND");
    let function = runtime
        .call_builtin(&mut ctx, coerce, &[type_of_symbol, function_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, function),
        ncl_object::ObjectRef::Function(_) | ncl_object::ObjectRef::Closure(_)
    ));
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[unbound, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let eql_form = list(&mut ctx, &runtime, &[eql, Word::fixnum(1)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(2), eql_form]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let array_form = list(&mut ctx, &runtime, &[array]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(2), array_form]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn parser_covers_empty_ranges_dimensions_and_invalid_forms() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let cons = intern(&mut ctx, &runtime, "COMMON-LISP", "CONS");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let integer_form = list(&mut ctx, &runtime, &[integer]);
    assert_eq!(
        parse_type_specifier(&mut ctx, integer_form).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Unbounded,
            high: IntegerBound::Unbounded,
        }
    );
    let rank = Word::fixnum(2);
    let array_form = list(&mut ctx, &runtime, &[array, star, rank]);
    assert_eq!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![],
            })),
            dimensions: Some(ArrayDimensions::Rank(2)),
            simple: false,
        }
    );
    let cons_form = list(&mut ctx, &runtime, &[cons, star, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, cons_form).unwrap(),
        TypeSpecifier::Cons {
            car: Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![],
            }),
            cdr: Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![],
            }),
        }
    );
    let function_form = list(&mut ctx, &runtime, &[function]);
    assert_eq!(
        parse_type_specifier(&mut ctx, function_form).unwrap(),
        TypeSpecifier::Function {
            lambda_list: vec![],
            return_type: Box::new(TypeSpecifier::Named(NamedType::T)),
        }
    );
    let too_many = list(&mut ctx, &runtime, &[vector, star, rank, rank]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, too_many),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let negative = list(&mut ctx, &runtime, &[array, star, Word::fixnum(-1)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, negative),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn typep_covers_numeric_bounds_composites_and_array_mismatches() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let array = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let character = Word::character(127);
    let exclusive = TypeSpecifier::IntegerRange {
        low: IntegerBound::Exclusive(0),
        high: IntegerBound::Exclusive(3),
    };
    assert!(typep(&mut ctx, Word::fixnum(1), &exclusive).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(0), &exclusive).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(3), &exclusive).unwrap());
    assert!(!typep(&mut ctx, character, &exclusive).unwrap());
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(2),
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::String),
                TypeSpecifier::Named(NamedType::Integer),
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(2),
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Named(NamedType::String),
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: true,
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
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Exact(2),
                    ArrayDimension::Exclusive(2),
                ])),
                simple: false,
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Member(vec![Value::Integer(2), Value::Character(65)])
        )
        .unwrap()
    );
}

#[test]
fn subtypep_exercises_boundary_relations_and_uncertainty() {
    let named = |name| TypeSpecifier::Named(name);
    let range = |low, high| TypeSpecifier::IntegerRange { low, high };
    assert_eq!(
        subtypep(
            &range(IntegerBound::Inclusive(1), IntegerBound::Exclusive(4)),
            &range(IntegerBound::Inclusive(0), IntegerBound::Inclusive(4)),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &range(IntegerBound::Exclusive(1), IntegerBound::Inclusive(4)),
            &range(IntegerBound::Exclusive(1), IntegerBound::Exclusive(5)),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &range(IntegerBound::Unbounded, IntegerBound::Inclusive(4)),
            &range(IntegerBound::Inclusive(0), IntegerBound::Inclusive(4)),
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::Number),
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &named(NamedType::Integer),
            &TypeSpecifier::Or(vec![named(NamedType::String), named(NamedType::Number)]),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::And(vec![named(NamedType::String), named(NamedType::Integer)]),
            &named(NamedType::Number),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Package), &named(NamedType::Stream)).unwrap(),
        (false, false)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Satisfies("WAVE5".to_owned()),
            &named(NamedType::T)
        ),
        Err(TypeError::CannotInvoke(name)) if name == "WAVE5"
    ));
}

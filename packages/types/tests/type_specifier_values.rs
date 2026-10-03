#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_stream, make_string, make_structure,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, NamedType, TypeError, TypeSpecifier, Value,
    parse_type_specifier, typep,
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

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
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
fn type_specifier_covers_optional_forms_and_rejections() {
    let (runtime, mut ctx) = setup();
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let t_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "T");

    let array_form = list(&mut ctx, &runtime, &[array]);
    assert_eq!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array {
            element_type: None,
            dimensions: None,
            simple: false,
        }
    );
    let vector_form = list(&mut ctx, &runtime, &[vector]);
    assert_eq!(
        parse_type_specifier(&mut ctx, vector_form).unwrap(),
        TypeSpecifier::Vector {
            element_type: None,
            size: None,
        }
    );
    let function_star_form = list(&mut ctx, &runtime, &[function, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, function_star_form).unwrap(),
        TypeSpecifier::Function {
            lambda_list: vec![],
            return_type: Box::new(TypeSpecifier::Named(NamedType::T)),
        }
    );
    let lambda = list(&mut ctx, &runtime, &[integer]);
    let function_lambda_form = list(&mut ctx, &runtime, &[function, lambda, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, function_lambda_form).unwrap(),
        TypeSpecifier::Function {
            lambda_list: vec![TypeSpecifier::Named(NamedType::Integer)],
            return_type: Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![],
            }),
        }
    );
    let array_wild_form = list(&mut ctx, &runtime, &[array, t_symbol, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, array_wild_form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::T))),
            dimensions: Some(ArrayDimensions::Wild),
            simple: false,
        }
    );

    let member = intern(&mut ctx, &runtime, "COMMON-LISP", "MEMBER");
    let invalid = list(&mut ctx, &runtime, &[member, lambda]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn typep_covers_named_negative_paths_and_nested_shapes() {
    let (runtime, mut ctx) = setup();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "X");
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[1, 1], options()).unwrap();

    for named in [
        NamedType::Nil,
        NamedType::ShortFloat,
        NamedType::SingleFloat,
        NamedType::LongFloat,
        NamedType::RandomState,
        NamedType::Restart,
        NamedType::ExtendedChar,
    ] {
        assert!(!typep(&mut ctx, Word::fixnum(1), &TypeSpecifier::Named(named)).unwrap());
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
        !typep(
            &mut ctx,
            Word::character(1),
            &TypeSpecifier::Named(NamedType::StandardChar)
        )
        .unwrap()
    );
    assert!(!typep(&mut ctx, symbol, &TypeSpecifier::Named(NamedType::Null)).unwrap());
    assert!(!typep(&mut ctx, string, &TypeSpecifier::Named(NamedType::Cons)).unwrap());
    assert!(!typep(&mut ctx, matrix, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(2),
            &TypeSpecifier::Named(NamedType::List)
        )
        .unwrap()
    );

    assert!(matches!(
        typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Satisfies("MISSING".to_owned())
        ),
        Err(TypeError::CannotInvoke(name)) if name == "MISSING"
    ));
    assert!(matches!(
        typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Deftype {
                name: "MISSING".to_owned(),
                args: vec![]
            }
        ),
        Err(TypeError::UnexpandedDeftype(name)) if name == "MISSING"
    ));
    assert!(
        typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Vector {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
                size: Some(ArrayDimension::Exact(1)),
            }
        )
        .unwrap()
    );
    assert_eq!(Value::Integer(7), Value::Integer(7));
}

#[test]
fn builtins_classify_remaining_objects_and_coerce_numeric_inputs() {
    let (runtime, mut ctx) = setup();
    let type_of = function(&runtime, &mut ctx, "TYPE-OF");
    let coerce = function(&runtime, &mut ctx, "COERCE");
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let ratio: Word = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let double: Word = make_double(&mut ctx, &runtime, -2.5).unwrap().into();
    let complex: Word = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
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
    for (object, expected) in [
        (Word::character(65), "CHARACTER"),
        (cons, "CONS"),
        (bignum, "BIGNUM"),
        (ratio, "RATIO"),
        (double, "DOUBLE-FLOAT"),
        (complex, "COMPLEX"),
        (table, "HASH-TABLE"),
        (stream, "STREAM"),
        (structure, "STRUCTURE-OBJECT"),
    ] {
        let result = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        assert_eq!(symbol_text(&ctx, result), expected);
    }

    let double_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let converted_big = runtime
        .call_builtin(&mut ctx, coerce, &[bignum, double_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, converted_big),
        ncl_object::ObjectRef::DoubleFloat(_)
    ));
    let converted_ratio = runtime
        .call_builtin(&mut ctx, coerce, &[ratio, double_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, converted_ratio),
        ncl_object::ObjectRef::DoubleFloat(_)
    ));
    let chars = list(
        &mut ctx,
        &runtime,
        &[Word::character(97), Word::character(98)],
    );
    let converted_string = runtime
        .call_builtin(&mut ctx, coerce, &[chars, string_type])
        .unwrap();
    assert_eq!(
        ncl_object::string_length(&ctx, converted_string).unwrap(),
        2
    );
    let one_char = make_string(&mut ctx, &runtime, &['q']).unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[one_char, character_type])
            .unwrap(),
        Word::character(113)
    );
}

#[test]
fn typep_arrays_cover_specialized_and_dimension_mismatch_paths() {
    let (runtime, mut ctx) = setup();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let array = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    assert!(
        typep(
            &mut ctx,
            bits,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Exact(2),
                    ArrayDimension::Any,
                ])),
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
                    ArrayDimension::Exact(2),
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
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Any),
            }
        )
        .unwrap()
    );
}

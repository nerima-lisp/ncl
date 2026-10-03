#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_specialized_array, make_string, make_structure,
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
fn typep_covers_boolean_symbol_sequence_and_object_kinds() {
    let (runtime, mut ctx) = setup();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE10");
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "WAVE10");
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    let layout = runtime.register_structure_layout(0).unwrap();
    let structure = make_structure(&mut ctx, &runtime, layout, &[]).unwrap();
    for (object, named) in [
        (Word::NIL, NamedType::Boolean),
        (Word::TRUE, NamedType::Boolean),
        (symbol, NamedType::Symbol),
        (keyword, NamedType::Keyword),
        (cons, NamedType::List),
        (cons, NamedType::Sequence),
        (vector, NamedType::Sequence),
        (string, NamedType::SimpleString),
        (table, NamedType::HashTable),
        (structure, NamedType::Structure),
        (Word::fixnum(3), NamedType::Atom),
        (Word::NIL, NamedType::Null),
    ] {
        assert!(
            typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap(),
            "{named:?}"
        );
    }
    for (object, named) in [
        (Word::fixnum(2), NamedType::Boolean),
        (Word::fixnum(2), NamedType::Symbol),
        (Word::NIL, NamedType::Keyword),
        (Word::fixnum(2), NamedType::Cons),
        (Word::NIL, NamedType::Structure),
    ] {
        assert!(
            !typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap(),
            "{named:?}"
        );
    }
}

#[test]
fn typep_covers_specialized_arrays_and_dimension_mismatches() {
    let (runtime, mut ctx) = setup();
    let bit = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let character = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Character,
        &[Word::character(65)],
    )
    .unwrap();
    let matrix = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    assert!(
        typep(
            &mut ctx,
            bit,
            &TypeSpecifier::Named(NamedType::SimpleBitVector)
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            character,
            &TypeSpecifier::Named(NamedType::BitVector)
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            character,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Character))),
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exact(1)])),
                simple: true,
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
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Wild),
                simple: false,
            }
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_type_of_and_coerce_malformed_or_unsupported_specs() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let string = make_string(&mut ctx, &runtime, &['a']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0)],
    )
    .unwrap();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    for (object, expected) in [
        (Word::NIL, "NULL"),
        (Word::character(65), "CHARACTER"),
        (cons, "CONS"),
        (string, "SIMPLE-STRING"),
        (vector, "SIMPLE-VECTOR"),
        (bits, "SIMPLE-BIT-VECTOR"),
    ] {
        let actual = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        let name = ncl_object::symbol_name(&ctx, actual).unwrap();
        let text: String = (0..ncl_object::string_length(&ctx, name).unwrap())
            .map(|index| ncl_object::string_ref(&ctx, name, index).unwrap())
            .collect();
        assert_eq!(text, expected);
    }
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let bad_form = list(&mut ctx, &runtime, &[integer, Word::TRUE]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(1), bad_form]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let empty = make_string(&mut ctx, &runtime, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[empty, character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn parser_covers_function_array_and_integer_malformed_forms() {
    let (runtime, mut ctx) = setup();
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let function_star = list(&mut ctx, &runtime, &[function, star, star]);
    assert_eq!(
        parse_type_specifier(&mut ctx, function_star).unwrap(),
        TypeSpecifier::Function {
            lambda_list: vec![],
            return_type: Box::new(TypeSpecifier::Deftype {
                name: "*".to_owned(),
                args: vec![],
            }),
        }
    );
    let rank = list(&mut ctx, &runtime, &[star]);
    let array_form = list(&mut ctx, &runtime, &[array, Word::TRUE, rank]);
    assert_eq!(
        parse_type_specifier(&mut ctx, array_form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::T))),
            dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Any])),
            simple: false,
        }
    );
    let malformed_integer = list(&mut ctx, &runtime, &[integer, Word::TRUE, star]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed_integer),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let malformed_array = list(
        &mut ctx,
        &runtime,
        &[array, Word::TRUE, Word::TRUE, Word::TRUE],
    );
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed_array),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_equal_top_bottom_ranges_and_uncertainty() {
    let named = |name| TypeSpecifier::Named(name);
    assert_eq!(
        subtypep(&named(NamedType::T), &named(NamedType::T)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::T), &named(NamedType::Integer)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Nil), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
    let range = |low, high| TypeSpecifier::IntegerRange { low, high };
    assert_eq!(
        subtypep(
            &range(IntegerBound::Unbounded, IntegerBound::Unbounded),
            &named(NamedType::Integer),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &range(IntegerBound::Inclusive(0), IntegerBound::Inclusive(4)),
            &range(IntegerBound::Exclusive(0), IntegerBound::Inclusive(4)),
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(&named(NamedType::Package), &named(NamedType::Integer)).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::String)]),
            &named(NamedType::Integer),
        )
        .unwrap(),
        (false, false)
    );
}

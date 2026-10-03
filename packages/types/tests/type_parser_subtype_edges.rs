#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_string,
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
fn typep_reaches_rank_one_and_rank_two_array_paths() {
    let (runtime, mut ctx) = setup();
    let rank_one = make_array(&mut ctx, &runtime, &[3], options()).unwrap();
    let rank_two = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let string = make_string(&mut ctx, &runtime, &['a']).unwrap();
    assert!(typep(&mut ctx, rank_one, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(!typep(&mut ctx, rank_two, &TypeSpecifier::Named(NamedType::Vector)).unwrap());
    assert!(typep(&mut ctx, rank_two, &TypeSpecifier::Named(NamedType::Array)).unwrap());
    assert!(
        !typep(
            &mut ctx,
            rank_two,
            &TypeSpecifier::Named(NamedType::SimpleArray)
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Vector {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Character))),
                size: Some(ArrayDimension::Exact(1)),
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Named(NamedType::BitVector)
        )
        .unwrap()
    );
}

#[test]
fn typep_short_circuits_compound_specs_and_matches_values() {
    let (runtime, mut ctx) = setup();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).unwrap();
    assert!(
        typep(
            &mut ctx,
            cons,
            &TypeSpecifier::Or(vec![
                TypeSpecifier::Named(NamedType::Cons),
                TypeSpecifier::Satisfies("not-called".to_owned()),
            ])
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::And(vec![
                TypeSpecifier::Named(NamedType::Integer),
                TypeSpecifier::Satisfies("not-called".to_owned()),
            ])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(7),
            &TypeSpecifier::Member(vec![Value::Integer(7), Value::Integer(8)])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::character(65),
            &TypeSpecifier::Eql(Value::Character(65))
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::character(66),
            &TypeSpecifier::Not(Box::new(TypeSpecifier::Eql(Value::Character(65))))
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_identity_coercions_and_character_errors() {
    let (runtime, mut ctx) = setup();
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let t_type = intern(&mut ctx, &runtime, "COMMON-LISP", "T");
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let string = make_string(&mut ctx, &runtime, &['z']).unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[Word::NIL, list_type])
            .unwrap(),
        Word::NIL
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[vector, vector_type])
            .unwrap(),
        vector
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, coerce, &[string, character_type])
            .unwrap(),
        Word::character(122)
    );
    let two_chars = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[two_chars, character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, t_type]),
        Ok(Word::NIL)
    );
}

#[test]
fn parser_reaches_simple_array_and_single_form_errors() {
    let (runtime, mut ctx) = setup();
    let simple_array = intern(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-ARRAY");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let dimensions = list(&mut ctx, &runtime, &[Word::fixnum(2), Word::fixnum(3)]);
    let form = list(&mut ctx, &runtime, &[simple_array, integer, dimensions]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
            dimensions: Some(ArrayDimensions::Ranks(vec![
                ArrayDimension::Exact(2),
                ArrayDimension::Exact(3),
            ])),
            simple: true,
        }
    );
    let not = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT");
    let invalid_not = list(&mut ctx, &runtime, &[not, integer, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_not),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let invalid_function = list(&mut ctx, &runtime, &[function, Word::fixnum(1)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_function),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let integer_range = list(
        &mut ctx,
        &runtime,
        &[integer, Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    );
    assert!(matches!(
        parse_type_specifier(&mut ctx, integer_range),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_remaining_range_bounds_and_error_operands() {
    let range = |low, high| TypeSpecifier::IntegerRange { low, high };
    let integer = TypeSpecifier::Named(NamedType::Integer);
    assert_eq!(
        subtypep(
            &range(IntegerBound::Inclusive(1), IntegerBound::Inclusive(3)),
            &range(IntegerBound::Exclusive(0), IntegerBound::Exclusive(4)),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &range(IntegerBound::Exclusive(1), IntegerBound::Exclusive(3)),
            &range(IntegerBound::Inclusive(1), IntegerBound::Inclusive(3)),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![
                integer.clone(),
                TypeSpecifier::Named(NamedType::String)
            ]),
            &integer,
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &integer,
            &TypeSpecifier::Or(vec![TypeSpecifier::Named(NamedType::Nil), integer.clone()]),
        )
        .unwrap(),
        (true, true)
    );
    assert!(matches!(
        subtypep(
            &integer,
            &TypeSpecifier::Satisfies("wave7".to_owned())
        ),
        Err(TypeError::CannotInvoke(name)) if name == "wave7"
    ));
    assert!(matches!(
        subtypep(
            &integer,
            &TypeSpecifier::Deftype {
                name: "wave7-type".to_owned(),
                args: vec![]
            }
        ),
        Err(TypeError::UnexpandedDeftype(name)) if name == "wave7-type"
    ));
}

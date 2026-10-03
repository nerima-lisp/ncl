#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_simple_vector, make_string,
};
use ncl_types::{
    ArrayDimension, ArrayDimensions, NamedType, TypeError, TypeSpecifier, Value,
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
fn typep_covers_rank_constraints_function_values_and_symbol_values() {
    let (runtime, mut ctx) = setup();
    let rank_one = make_array(&mut ctx, &runtime, &[2], options()).unwrap();
    let rank_two = make_array(&mut ctx, &runtime, &[2, 2], options()).unwrap();
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    let symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE9");
    let string = make_string(&mut ctx, &runtime, &['W', '9']).unwrap();

    assert!(
        typep(
            &mut ctx,
            rank_one,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exact(2)])),
                simple: false,
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            rank_two,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Ranks(vec![ArrayDimension::Exact(2)])),
                simple: false,
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            function,
            &TypeSpecifier::Function {
                lambda_list: vec![TypeSpecifier::Named(NamedType::Integer)],
                return_type: Box::new(TypeSpecifier::Named(NamedType::String)),
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Function {
                lambda_list: vec![],
                return_type: Box::new(TypeSpecifier::Named(NamedType::T)),
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(4),
            &TypeSpecifier::Values(vec![TypeSpecifier::Named(NamedType::Integer)])
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            symbol,
            &TypeSpecifier::Eql(Value::Symbol("WAVE9".to_owned()))
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Eql(Value::String("W9".to_owned()))
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_type_of_function_and_unsupported_coercions() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let coerce = builtin(&runtime, &mut ctx, "COERCE");
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    let type_name = runtime
        .call_builtin(&mut ctx, type_of, &[function])
        .unwrap();
    let expected = intern(&mut ctx, &runtime, "COMMON-LISP", "COMPILED-FUNCTION");
    assert_eq!(type_name, expected);
    let t = intern(&mut ctx, &runtime, "COMMON-LISP", "T");
    assert_eq!(
        runtime.call_builtin(&mut ctx, type_of, &[Word::UNBOUND]),
        Ok(t)
    );

    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let integer_range = list(
        &mut ctx,
        &runtime,
        &[integer, Word::fixnum(0), Word::fixnum(3)],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(4), integer_range]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let values_form = list(&mut ctx, &runtime, &[eql, Word::fixnum(1)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(2), values_form]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let array_form = list(&mut ctx, &runtime, &[array]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[string, array_form]),
        Ok(string)
    );
}

#[test]
fn parser_covers_unknown_deftype_values_and_dimension_errors() {
    let (runtime, mut ctx) = setup();
    let custom = intern(&mut ctx, &runtime, "COMMON-LISP", "WAVE9-TYPE");
    let custom_form = list(&mut ctx, &runtime, &[custom, Word::fixnum(8), Word::TRUE]);
    assert_eq!(
        parse_type_specifier(&mut ctx, custom_form).unwrap(),
        TypeSpecifier::Deftype {
            name: "WAVE9-TYPE".to_owned(),
            args: vec![Value::Integer(8), Value::True],
        }
    );
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let invalid_dimensions = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::TRUE]);
    let invalid_array = list(&mut ctx, &runtime, &[array, Word::TRUE, invalid_dimensions]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_array),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let vector = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let invalid_vector = list(&mut ctx, &runtime, &[vector, Word::TRUE, Word::TRUE]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_vector),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let malformed_low = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]);
    let malformed_bound = list(&mut ctx, &runtime, &[integer, malformed_low, Word::TRUE]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, malformed_bound),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn subtypep_covers_empty_composites_and_uncertain_named_relations() {
    let named = |name| TypeSpecifier::Named(name);
    assert_eq!(
        subtypep(&TypeSpecifier::Or(vec![]), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&TypeSpecifier::And(vec![]), &named(NamedType::Integer)).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(
            &TypeSpecifier::Or(vec![named(NamedType::Integer), named(NamedType::Fixnum)]),
            &named(NamedType::Integer),
        )
        .unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Package), &named(NamedType::Symbol)).unwrap(),
        (false, false)
    );
    assert!(matches!(
        subtypep(
            &TypeSpecifier::Deftype {
                name: "WAVE9".to_owned(),
                args: vec![],
            },
            &named(NamedType::T),
        ),
        Err(TypeError::UnexpandedDeftype(name)) if name == "WAVE9"
    ));
    assert!(matches!(
        subtypep(
            &named(NamedType::Integer),
            &TypeSpecifier::Satisfies("WAVE9-PRED".to_owned()),
        ),
        Err(TypeError::CannotInvoke(name)) if name == "WAVE9-PRED"
    ));
}

#[test]
fn typep_array_and_not_paths_reject_wrong_rank_and_wrong_kind() {
    let (runtime, mut ctx) = setup();
    let rank_two = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert!(
        typep(
            &mut ctx,
            rank_two,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::String))),
                dimensions: Some(ArrayDimensions::Ranks(vec![
                    ArrayDimension::Any,
                    ArrayDimension::Exclusive(4),
                ])),
                simple: false,
            }
        )
        .unwrap()
    );
    assert!(
        !typep(
            &mut ctx,
            vector,
            &TypeSpecifier::Array {
                element_type: None,
                dimensions: Some(ArrayDimensions::Rank(2)),
                simple: false,
            }
        )
        .unwrap()
    );
    assert!(
        typep(
            &mut ctx,
            Word::fixnum(1),
            &TypeSpecifier::Not(Box::new(TypeSpecifier::Named(NamedType::String)))
        )
        .unwrap()
    );
}

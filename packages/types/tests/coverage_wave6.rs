#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_stream, make_string,
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
fn typep_checks_runtime_kinds_and_nested_cons_failures() {
    let (runtime, mut ctx) = setup();
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "TYPE-OF")
        .unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
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
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0)],
    )
    .unwrap();
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
    let cases = [
        (package, NamedType::Package),
        (function, NamedType::Function),
        (function, NamedType::CompiledFunction),
        (bignum, NamedType::Integer),
        (bignum, NamedType::Bignum),
        (ratio, NamedType::Rational),
        (ratio, NamedType::Ratio),
        (float, NamedType::Real),
        (float, NamedType::DoubleFloat),
        (complex, NamedType::Number),
        (complex, NamedType::Complex),
        (string, NamedType::SimpleArray),
        (vector, NamedType::SimpleVector),
        (bits, NamedType::SimpleBitVector),
        (stream, NamedType::Stream),
        (Word::fixnum(1), NamedType::Atom),
        (Word::fixnum(1), NamedType::ValuesType),
    ];
    for (object, named) in cases {
        assert!(
            typep(&mut ctx, object, &TypeSpecifier::Named(named)).unwrap(),
            "{named:?}"
        );
    }

    let wrong_car = make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL).unwrap();
    let wrong_cdr = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::TRUE).unwrap();
    let cons_of_integers = TypeSpecifier::Cons {
        car: Box::new(TypeSpecifier::Named(NamedType::Integer)),
        cdr: Box::new(TypeSpecifier::Named(NamedType::Null)),
    };
    assert!(!typep(&mut ctx, wrong_car, &cons_of_integers).unwrap());
    assert!(!typep(&mut ctx, wrong_cdr, &cons_of_integers).unwrap());
    assert!(
        !typep(
            &mut ctx,
            Word::NIL,
            &TypeSpecifier::Vector {
                element_type: None,
                size: Some(ArrayDimension::Exact(0)),
            }
        )
        .unwrap()
    );
}

#[test]
fn builtins_cover_wildcard_type_and_subtypep_multiple_values() {
    let (runtime, mut ctx) = setup();
    let type_of = builtin(&runtime, &mut ctx, "TYPE-OF");
    let subtypep_builtin = builtin(&runtime, &mut ctx, "SUBTYPEP");
    let unknown = runtime
        .call_builtin(&mut ctx, type_of, &[Word::UNBOUND])
        .unwrap();
    let t_symbol = intern(&mut ctx, &runtime, "COMMON-LISP", "T");
    assert_eq!(unknown, t_symbol);

    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let number = intern(&mut ctx, &runtime, "COMMON-LISP", "NUMBER");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, subtypep_builtin, &[integer, number])
            .unwrap(),
        Word::TRUE
    );
    assert_eq!(ctx.values(), &[Word::TRUE]);

    let invalid = Word::fixnum(12);
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, subtypep_builtin, &[invalid, integer])
            .unwrap(),
        Word::NIL
    );
    assert_eq!(ctx.values(), &[Word::NIL]);
}

#[test]
fn parser_rejects_non_values_and_accepts_dimension_lists() {
    let (runtime, mut ctx) = setup();
    assert!(matches!(
        parse_type_specifier(&mut ctx, Word::fixnum(1)),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let array = intern(&mut ctx, &runtime, "COMMON-LISP", "ARRAY");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let star = intern(&mut ctx, &runtime, "COMMON-LISP", "*");
    let dimensions = list(&mut ctx, &runtime, &[star, Word::fixnum(3)]);
    let form = list(&mut ctx, &runtime, &[array, integer, dimensions]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::Integer))),
            dimensions: Some(ArrayDimensions::Ranks(vec![
                ArrayDimension::Any,
                ArrayDimension::Exact(3),
            ])),
            simple: false,
        }
    );
    let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
    let invalid_eql = list(&mut ctx, &runtime, &[eql, integer]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_eql),
        Err(TypeError::InvalidSpecifier(_))
    ));
    let satisfies = intern(&mut ctx, &runtime, "COMMON-LISP", "SATISFIES");
    let invalid_satisfies = list(&mut ctx, &runtime, &[satisfies, Word::fixnum(4)]);
    assert!(matches!(
        parse_type_specifier(&mut ctx, invalid_satisfies),
        Err(TypeError::InvalidSpecifier(_))
    ));
}

#[test]
fn typep_array_dimensions_and_value_identity_have_negative_results() {
    let (runtime, mut ctx) = setup();
    let array = make_array(&mut ctx, &runtime, &[2, 3], options()).unwrap();
    assert!(
        typep(
            &mut ctx,
            array,
            &TypeSpecifier::Array {
                element_type: Some(Box::new(TypeSpecifier::Named(NamedType::String))),
                dimensions: Some(ArrayDimensions::Wild),
                simple: false,
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
                dimensions: Some(ArrayDimensions::Rank(1)),
                simple: false,
            }
        )
        .unwrap()
    );
    let value = Value::String("different".to_owned());
    let string = make_string(&mut ctx, &runtime, &['d', 'i', 'f']).unwrap();
    assert!(!typep(&mut ctx, string, &TypeSpecifier::Eql(value)).unwrap());
    assert!(
        typep(
            &mut ctx,
            string,
            &TypeSpecifier::Member(vec![Value::String("dif".to_owned())])
        )
        .unwrap()
    );
}

#[test]
fn subtypep_handles_top_bottom_disjoint_and_and_shortcuts() {
    let named = |name| TypeSpecifier::Named(name);
    assert_eq!(
        subtypep(&named(NamedType::Nil), &named(NamedType::String)).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Integer), &named(NamedType::Nil)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Complex), &named(NamedType::Real)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(NamedType::Cons), &named(NamedType::Symbol)).unwrap(),
        (false, true)
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
            &named(NamedType::Integer),
            &TypeSpecifier::Or(vec![named(NamedType::String), named(NamedType::Float)]),
        )
        .unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(&named(NamedType::Integer), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
}

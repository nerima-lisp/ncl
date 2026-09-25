#![allow(clippy::unwrap_used, reason = "tests assert on parse results")]

//! Round-trip tests for type-specifier parsing.

use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons};
use ncl_types::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeSpecifier, Value,
    parse_type_specifier,
};

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package).intern(ctx, runtime, name).unwrap().0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, items: &[Word]) -> Word {
    let mut result = Word::NIL;
    for &item in items.iter().rev() {
        result = make_cons(ctx, runtime, item, result).unwrap();
    }
    result
}

#[test]
fn named_type_names_resolve() {
    assert_eq!(NamedType::from_name("INTEGER"), Some(NamedType::Integer));
    assert_eq!(
        NamedType::from_name("SIMPLE-ARRAY"),
        Some(NamedType::SimpleArray)
    );
    assert_eq!(NamedType::from_name("VALUES"), Some(NamedType::ValuesType));
    assert_eq!(NamedType::from_name("NOT-A-TYPE"), None);
}

#[test]
fn nil_and_t_parse_as_named() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(
        parse_type_specifier(&mut ctx, Word::NIL).unwrap(),
        TypeSpecifier::Named(NamedType::Nil)
    );
    assert_eq!(
        parse_type_specifier(&mut ctx, Word::TRUE).unwrap(),
        TypeSpecifier::Named(NamedType::T)
    );
}

#[test]
fn symbol_parses_to_named() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let integer = intern(&mut ctx, &runtime, "INTEGER");
    assert_eq!(
        parse_type_specifier(&mut ctx, integer).unwrap(),
        TypeSpecifier::Named(NamedType::Integer)
    );
}

#[test]
fn unknown_symbol_parses_to_deftype() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let mine = intern(&mut ctx, &runtime, "MY-TYPE");
    let parsed = parse_type_specifier(&mut ctx, mine).unwrap();
    assert!(matches!(
        parsed,
        TypeSpecifier::Deftype { name, args } if name == "MY-TYPE" && args.is_empty()
    ));
}

#[test]
fn integer_range_round_trips() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let integer = intern(&mut ctx, &runtime, "INTEGER");
    let zero = Word::fixnum(0);
    let ten = Word::fixnum(10);
    let form = list(&mut ctx, &runtime, &[integer, zero, ten]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Inclusive(0),
            high: IntegerBound::Inclusive(10)
        }
    );
}

#[test]
fn or_and_not_member_parse() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let or = intern(&mut ctx, &runtime, "OR");
    let and = intern(&mut ctx, &runtime, "AND");
    let not = intern(&mut ctx, &runtime, "NOT");
    let member = intern(&mut ctx, &runtime, "MEMBER");
    let symbol = intern(&mut ctx, &runtime, "SYMBOL");
    let one = Word::fixnum(1);

    let or_form = list(&mut ctx, &runtime, &[or, symbol, Word::TRUE]);
    assert_eq!(
        parse_type_specifier(&mut ctx, or_form).unwrap(),
        TypeSpecifier::Or(vec![
            TypeSpecifier::Named(NamedType::Symbol),
            TypeSpecifier::Named(NamedType::T)
        ])
    );

    let not_form = list(&mut ctx, &runtime, &[not, symbol]);
    assert_eq!(
        parse_type_specifier(&mut ctx, not_form).unwrap(),
        TypeSpecifier::Not(Box::new(TypeSpecifier::Named(NamedType::Symbol)))
    );

    let member_form = list(&mut ctx, &runtime, &[member, one, Word::NIL]);
    assert_eq!(
        parse_type_specifier(&mut ctx, member_form).unwrap(),
        TypeSpecifier::Member(vec![Value::Integer(1), Value::Nil])
    );

    let and_form = list(&mut ctx, &runtime, &[and]);
    assert_eq!(
        parse_type_specifier(&mut ctx, and_form).unwrap(),
        TypeSpecifier::And(vec![])
    );
}

#[test]
fn array_dimensions_parse() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let array = intern(&mut ctx, &runtime, "ARRAY");
    let simple_array = intern(&mut ctx, &runtime, "SIMPLE-ARRAY");
    let t_sym = intern(&mut ctx, &runtime, "T");
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);

    let form = list(&mut ctx, &runtime, &[array, t_sym, two]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::T))),
            dimensions: Some(ArrayDimensions::Rank(2)),
            simple: false,
        }
    );

    let dims = list(&mut ctx, &runtime, &[two, three]);
    let form = list(&mut ctx, &runtime, &[simple_array, t_sym, dims]);
    assert_eq!(
        parse_type_specifier(&mut ctx, form).unwrap(),
        TypeSpecifier::Array {
            element_type: Some(Box::new(TypeSpecifier::Named(NamedType::T))),
            dimensions: Some(ArrayDimensions::Ranks(vec![
                ArrayDimension::Exact(2),
                ArrayDimension::Exact(3)
            ])),
            simple: true,
        }
    );
}

#[test]
fn cons_and_function_parse() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let cons = intern(&mut ctx, &runtime, "CONS");
    let symbol = intern(&mut ctx, &runtime, "SYMBOL");
    let function = intern(&mut ctx, &runtime, "FUNCTION");

    let cons_form = list(&mut ctx, &runtime, &[cons, symbol]);
    assert_eq!(
        parse_type_specifier(&mut ctx, cons_form).unwrap(),
        TypeSpecifier::Cons {
            car: Box::new(TypeSpecifier::Named(NamedType::Symbol)),
            cdr: Box::new(TypeSpecifier::Named(NamedType::T)),
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
}

#[test]
fn malformed_arity_is_rejected() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let not = intern(&mut ctx, &runtime, "NOT");
    let integer = intern(&mut ctx, &runtime, "INTEGER");
    let cons = intern(&mut ctx, &runtime, "CONS");
    let function = intern(&mut ctx, &runtime, "FUNCTION");
    let array = intern(&mut ctx, &runtime, "ARRAY");
    let symbol = intern(&mut ctx, &runtime, "SYMBOL");
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);

    let cases = [
        (
            "(not symbol symbol)",
            list(&mut ctx, &runtime, &[not, symbol, symbol]),
        ),
        (
            "(integer 1 2 3)",
            list(&mut ctx, &runtime, &[integer, one, two, three]),
        ),
        (
            "(cons symbol symbol symbol)",
            list(&mut ctx, &runtime, &[cons, symbol, symbol, symbol]),
        ),
        (
            "(function nil symbol symbol)",
            list(&mut ctx, &runtime, &[function, Word::NIL, symbol, symbol]),
        ),
        (
            "(array symbol 2 3)",
            list(&mut ctx, &runtime, &[array, symbol, two, three]),
        ),
    ];
    for (label, form) in cases {
        assert!(
            matches!(
                parse_type_specifier(&mut ctx, form),
                Err(ncl_types::TypeError::InvalidSpecifier(_))
            ),
            "expected rejection for {label}"
        );
    }
}

#[test]
fn non_symbol_non_list_is_invalid() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let five = Word::fixnum(5);
    assert!(matches!(
        parse_type_specifier(&mut ctx, five),
        Err(ncl_types::TypeError::InvalidSpecifier(word)) if word == five
    ));
}

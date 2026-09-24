#![allow(clippy::unwrap_used, reason = "tests assert on type predicates")]

//! Type predicates for each implemented type.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, Package, Runtime, ThreadContext, Word, make_array, make_cons,
    make_double, make_simple_vector, make_specialized_array, make_string,
};
use ncl_types::{NamedType, TypeSpecifier, typep};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

const fn named(name: NamedType) -> TypeSpecifier {
    TypeSpecifier::Named(name)
}

#[test]
fn top_bottom_and_boolean() {
    let (runtime, mut ctx) = setup();
    let _ = &runtime;
    assert!(typep(&mut ctx, Word::NIL, &named(NamedType::T)).unwrap());
    assert!(!typep(&mut ctx, Word::NIL, &named(NamedType::Nil)).unwrap());
    assert!(typep(&mut ctx, Word::NIL, &named(NamedType::Boolean)).unwrap());
    assert!(typep(&mut ctx, Word::TRUE, &named(NamedType::Boolean)).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(1), &named(NamedType::Boolean)).unwrap());
}

#[test]
fn fixnum_and_numeric_tower() {
    let (runtime, mut ctx) = setup();
    let five = Word::fixnum(5);
    assert!(typep(&mut ctx, five, &named(NamedType::Fixnum)).unwrap());
    assert!(typep(&mut ctx, five, &named(NamedType::Integer)).unwrap());
    assert!(typep(&mut ctx, five, &named(NamedType::Rational)).unwrap());
    assert!(typep(&mut ctx, five, &named(NamedType::Real)).unwrap());
    assert!(typep(&mut ctx, five, &named(NamedType::Number)).unwrap());
    assert!(!typep(&mut ctx, five, &named(NamedType::Float)).unwrap());
    assert!(!typep(&mut ctx, five, &named(NamedType::Complex)).unwrap());
    assert!(!typep(&mut ctx, five, &named(NamedType::String)).unwrap());

    let big = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    assert!(typep(&mut ctx, big, &named(NamedType::Integer)).unwrap());
    assert!(!typep(&mut ctx, big, &named(NamedType::Fixnum)).unwrap());
}

#[test]
fn double_float_and_complex() {
    let (runtime, mut ctx) = setup();
    let double = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    assert!(typep(&mut ctx, double, &named(NamedType::DoubleFloat)).unwrap());
    assert!(typep(&mut ctx, double, &named(NamedType::Float)).unwrap());
    assert!(typep(&mut ctx, double, &named(NamedType::Real)).unwrap());
    assert!(!typep(&mut ctx, double, &named(NamedType::Integer)).unwrap());

    let complex = ncl_object::make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    assert!(typep(&mut ctx, complex, &named(NamedType::Complex)).unwrap());
    assert!(typep(&mut ctx, complex, &named(NamedType::Number)).unwrap());
    assert!(!typep(&mut ctx, complex, &named(NamedType::Real)).unwrap());
}

#[test]
fn cons_list_and_null() {
    let (runtime, mut ctx) = setup();
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    assert!(typep(&mut ctx, cons, &named(NamedType::Cons)).unwrap());
    assert!(typep(&mut ctx, cons, &named(NamedType::List)).unwrap());
    assert!(typep(&mut ctx, cons, &named(NamedType::Sequence)).unwrap());
    assert!(typep(&mut ctx, Word::NIL, &named(NamedType::List)).unwrap());
    assert!(typep(&mut ctx, Word::NIL, &named(NamedType::Null)).unwrap());
    assert!(typep(&mut ctx, Word::NIL, &named(NamedType::Symbol)).unwrap());
    assert!(!typep(&mut ctx, Word::NIL, &named(NamedType::Cons)).unwrap());
}

#[test]
fn symbol_and_keyword() {
    let (runtime, mut ctx) = setup();
    let symbol = {
        let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
        Package::from(package)
            .intern(&mut ctx, &runtime, "FOO")
            .unwrap()
            .0
    };
    assert!(typep(&mut ctx, symbol, &named(NamedType::Symbol)).unwrap());
    assert!(!typep(&mut ctx, symbol, &named(NamedType::Keyword)).unwrap());

    let keyword = {
        let package = runtime.find_package(&ctx, "KEYWORD").unwrap();
        Package::from(package)
            .intern(&mut ctx, &runtime, "FOO")
            .unwrap()
            .0
    };
    assert!(typep(&mut ctx, keyword, &named(NamedType::Symbol)).unwrap());
    assert!(typep(&mut ctx, keyword, &named(NamedType::Keyword)).unwrap());
}

#[test]
fn string_vector_array_and_bit_vector() {
    let (runtime, mut ctx) = setup();
    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    assert!(typep(&mut ctx, string, &named(NamedType::String)).unwrap());
    assert!(typep(&mut ctx, string, &named(NamedType::Vector)).unwrap());
    assert!(typep(&mut ctx, string, &named(NamedType::Array)).unwrap());
    assert!(typep(&mut ctx, string, &named(NamedType::Sequence)).unwrap());

    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert!(typep(&mut ctx, vector, &named(NamedType::SimpleVector)).unwrap());
    assert!(typep(&mut ctx, vector, &named(NamedType::Vector)).unwrap());
    assert!(typep(&mut ctx, vector, &named(NamedType::Array)).unwrap());

    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    assert!(typep(&mut ctx, bits, &named(NamedType::BitVector)).unwrap());
    assert!(typep(&mut ctx, bits, &named(NamedType::Vector)).unwrap());

    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert!(typep(&mut ctx, array, &named(NamedType::Array)).unwrap());
    assert!(!typep(&mut ctx, array, &named(NamedType::SimpleArray)).unwrap());
}

#[test]
fn hash_table() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    assert!(typep(&mut ctx, table, &named(NamedType::HashTable)).unwrap());
}

#[test]
fn integer_range() {
    let (runtime, mut ctx) = setup();
    let _ = &runtime;
    let spec = TypeSpecifier::IntegerRange {
        low: Some(Word::fixnum(0)),
        high: Some(Word::fixnum(10)),
    };
    assert!(typep(&mut ctx, Word::fixnum(5), &spec).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(0), &spec).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(10), &spec).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(11), &spec).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(-1), &spec).unwrap());
}

#[test]
fn compound_and_or_not_member_eql() {
    let (runtime, mut ctx) = setup();
    let _ = &runtime;
    let one = Word::fixnum(1);
    let integer = named(NamedType::Integer);
    let string = named(NamedType::String);

    let or = TypeSpecifier::Or(vec![integer, string]);
    assert!(typep(&mut ctx, one, &or).unwrap());

    let and = TypeSpecifier::And(vec![named(NamedType::Number), named(NamedType::Integer)]);
    assert!(typep(&mut ctx, one, &and).unwrap());

    let not = TypeSpecifier::Not(Box::new(named(NamedType::Cons)));
    assert!(typep(&mut ctx, one, &not).unwrap());

    let member = TypeSpecifier::Member(vec![one, Word::fixnum(2)]);
    assert!(typep(&mut ctx, one, &member).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(3), &member).unwrap());

    let eql = TypeSpecifier::Eql(one);
    assert!(typep(&mut ctx, one, &eql).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(2), &eql).unwrap());
}

#[test]
fn array_and_vector_dimension_checks() {
    let (runtime, mut ctx) = setup();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let sized = TypeSpecifier::Vector {
        element_type: None,
        size: Some(Word::fixnum(2)),
    };
    assert!(typep(&mut ctx, vector, &sized).unwrap());

    let wrong = TypeSpecifier::Vector {
        element_type: None,
        size: Some(Word::fixnum(3)),
    };
    assert!(!typep(&mut ctx, vector, &wrong).unwrap());

    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let dims = TypeSpecifier::Array {
        element_type: None,
        dimensions: Some(ncl_types::ArrayDimensions::Ranks(vec![
            Some(Word::fixnum(2)),
            Some(Word::fixnum(3)),
        ])),
        simple: false,
    };
    assert!(typep(&mut ctx, array, &dims).unwrap());
}

#[test]
fn satisfies_and_deftype_error() {
    let (runtime, mut ctx) = setup();
    let _ = &runtime;
    let predicate = {
        let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
        Package::from(package)
            .intern(&mut ctx, &runtime, "PRED")
            .unwrap()
            .0
    };
    let satisfies = TypeSpecifier::Satisfies(predicate);
    assert!(matches!(
        typep(&mut ctx, Word::NIL, &satisfies),
        Err(ncl_types::TypeError::CannotInvoke(word)) if word == predicate
    ));

    let deftype = TypeSpecifier::Deftype {
        name: predicate,
        args: vec![],
    };
    assert!(matches!(
        typep(&mut ctx, Word::NIL, &deftype),
        Err(ncl_types::TypeError::UnexpandedDeftype(word)) if word == predicate
    ));
}

#![allow(clippy::unwrap_used, reason = "tests assert on subtype results")]

//! Representative `subtypep` cases, including uncertain ones.

use ncl_object::Word;
use ncl_types::{NamedType, TypeSpecifier, subtypep};

const fn named(name: NamedType) -> TypeSpecifier {
    TypeSpecifier::Named(name)
}

fn range(low: Option<i64>, high: Option<i64>) -> TypeSpecifier {
    TypeSpecifier::IntegerRange {
        low: low.map(Word::fixnum),
        high: high.map(Word::fixnum),
    }
}

#[test]
fn numeric_tower() {
    use NamedType::{Bignum, Complex, Fixnum, Float, Integer, Number, Rational, Real};
    let cases = [
        (Fixnum, Integer, (true, true)),
        (Bignum, Integer, (true, true)),
        (Integer, Rational, (true, true)),
        (Rational, Real, (true, true)),
        (Real, Number, (true, true)),
        (Integer, Number, (true, true)),
        (Integer, Float, (false, true)),
        (Fixnum, Float, (false, false)),
        (Real, Complex, (false, true)),
    ];
    for (sub, sup, expected) in cases {
        assert_eq!(
            subtypep(&named(sub), &named(sup)).unwrap(),
            expected,
            "subtypep({sub:?}, {sup:?})"
        );
    }
}

#[test]
fn top_and_bottom() {
    use NamedType::{Integer, Nil, T};
    assert_eq!(
        subtypep(&named(Nil), &named(Integer)).unwrap(),
        (true, true)
    );
    assert_eq!(subtypep(&named(Integer), &named(T)).unwrap(), (true, true));
    assert_eq!(subtypep(&named(T), &named(Integer)).unwrap(), (false, true));
    assert_eq!(
        subtypep(&named(Integer), &named(Nil)).unwrap(),
        (false, true)
    );
    assert_eq!(
        subtypep(&named(Integer), &named(Integer)).unwrap(),
        (true, true)
    );
}

#[test]
fn integer_range_containment() {
    assert_eq!(
        subtypep(&range(Some(0), Some(10)), &range(Some(0), Some(20))).unwrap(),
        (true, true)
    );
    assert_eq!(
        subtypep(&range(Some(0), Some(10)), &range(Some(5), Some(15))).unwrap(),
        (false, false)
    );
    assert_eq!(
        subtypep(&range(Some(0), Some(10)), &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
}

#[test]
fn or_and_combinations() {
    let or = TypeSpecifier::Or(vec![named(NamedType::Fixnum), named(NamedType::Bignum)]);
    assert_eq!(
        subtypep(&or, &named(NamedType::Integer)).unwrap(),
        (true, true)
    );

    let and = TypeSpecifier::And(vec![named(NamedType::Number), named(NamedType::Integer)]);
    assert_eq!(
        subtypep(&and, &named(NamedType::Integer)).unwrap(),
        (true, true)
    );
}

#[test]
fn deftype_and_satisfies_error() {
    let deftype = TypeSpecifier::Deftype {
        name: Word::fixnum(1),
        args: vec![],
    };
    assert!(matches!(
        subtypep(&deftype, &named(NamedType::T)),
        Err(ncl_types::TypeError::UnexpandedDeftype(_))
    ));

    let satisfies = TypeSpecifier::Satisfies(Word::fixnum(1));
    assert!(matches!(
        subtypep(&satisfies, &named(NamedType::T)),
        Err(ncl_types::TypeError::CannotInvoke(_))
    ));
}

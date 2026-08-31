use super::{literal_constant, literals::rational_literal_parts};
use crate::Constant;

#[test]
fn literal_constants_are_parsed_from_a_table() {
    let cases = [
        ("nil", Constant::Nil),
        ("#t", Constant::Boolean(true)),
        (":ready", Constant::Keyword("READY".to_string())),
        (
            "1/2",
            Constant::Rational {
                numerator: 1,
                denominator: 2,
            },
        ),
        (
            "170141183460469231731687303715884105728/3",
            Constant::BigRational {
                numerator: "170141183460469231731687303715884105728".to_string(),
                denominator: "3".to_string(),
            },
        ),
        ("-6/3", Constant::Integer(-2)),
        ("#xFF", Constant::Integer(255)),
        ("#b1010", Constant::Integer(10)),
        ("#o777", Constant::Integer(511)),
        ("#3r120", Constant::Integer(15)),
        ("1.25s0", Constant::Float(1.25)),
    ];
    for (source, expected) in cases {
        assert_eq!(
            literal_constant(source),
            Some(expected),
            "source={source:?}"
        );
    }
}

#[test]
fn rational_literals_cover_invalid_and_reduced_forms() {
    let cases = [
        ("6/8", Some((3, 4))),
        ("6/-8", Some((-3, 4))),
        ("0/9", Some((0, 1))),
        ("1/0", None),
        ("1/2/3", None),
        ("9223372036854775808/1", None),
        ("6x/8", None),
        ("6/8x", None),
        ("1/9223372036854775808", None),
        ("-170141183460469231731687303715884105728/1", None),
        ("-170141183460469231731687303715884105728/-1", None),
        ("1/-170141183460469231731687303715884105728", None),
    ];
    for (source, expected) in cases {
        assert_eq!(
            rational_literal_parts(source),
            expected,
            "source={source:?}"
        );
    }
}

#[test]
fn literal_constant_rejects_a_token_that_fails_to_parse() {
    assert_eq!(literal_constant(""), None);
}

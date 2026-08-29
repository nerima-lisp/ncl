#![allow(clippy::wildcard_imports)]

use crate::Value;
use crate::builtins::*;

fn assert_value(result: Result<Value, crate::RuntimeError>, expected: impl std::fmt::Display) {
    assert_eq!(
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("builtin should succeed: {error}"),
        },
        expected.to_string()
    );
}

#[test]
fn handles_sequence_transforms_and_bounds() {
    let string = Value::string("AbC");
    assert_value(
        string_upcase(std::slice::from_ref(&string)),
        Value::string("ABC"),
    );
    assert_value(
        string_downcase(std::slice::from_ref(&string)),
        Value::string("abc"),
    );
    assert_value(
        string_capitalize(&[Value::string("hello WORLD")]),
        Value::string("Hello World"),
    );
    assert_value(
        subseq(&[string.clone(), Value::Integer(1), Value::Integer(3)]),
        Value::string("bC"),
    );
    assert!(subseq(&[string, Value::Integer(3), Value::Integer(1)]).is_err());
    assert_value(
        length(&[Value::list(vec![Value::Nil, Value::Nil])]),
        Value::Integer(2),
    );
    assert!(elt(&[Value::string("a"), Value::Integer(2)]).is_err());
    assert_value(
        elt(&[Value::list(vec![Value::Integer(4)]), Value::Integer(0)]),
        Value::Integer(4),
    );
    assert_value(
        elt(&[Value::vector(vec![Value::Integer(5)]), Value::Integer(0)]),
        Value::Integer(5),
    );
    assert!(elt(&[Value::Nil, Value::Integer(0)]).is_err());
    assert!(elt(&[Value::Integer(1), Value::Integer(0)]).is_err());
}

#[test]
fn handles_string_comparisons_and_type_predicates() {
    assert_value(
        string_equal(&[Value::string("abc"), Value::string("abc")]),
        Value::boolean(true),
    );
    assert_value(
        string_case_equal(&[Value::string("AbC"), Value::string("aBc")]),
        Value::boolean(true),
    );
    assert_value(
        string_less_than(&[Value::string("a"), Value::string("b")]),
        Value::Integer(0),
    );
    assert_value(
        string_greater_than(&[Value::string("b"), Value::string("a")]),
        Value::Integer(0),
    );
    assert_value(
        string_less_equal(&[Value::string("a"), Value::string("a")]),
        Value::Integer(1),
    );
    assert_value(
        string_greater_equal(&[Value::string("b"), Value::string("a")]),
        Value::Integer(0),
    );
    assert_value(
        string_less_equal(&[Value::string("b"), Value::string("a")]),
        Value::Nil,
    );
    assert_value(
        string_greater_equal(&[Value::string("a"), Value::string("b")]),
        Value::Nil,
    );
    assert_value(characterp(&[Value::Character('x')]), Value::boolean(true));
    assert_value(keywordp(&[Value::keyword("name")]), Value::boolean(true));
    assert_value(
        vectorp(&[Value::vector(vec![Value::Nil])]),
        Value::boolean(true),
    );
    assert_value(endp(&[Value::Nil]), Value::boolean(true));
    assert!(characterp(&[Value::Integer(1), Value::Integer(2)]).is_err());
    assert!(string_equal(&[Value::string("a"), Value::Integer(1)]).is_err());
}

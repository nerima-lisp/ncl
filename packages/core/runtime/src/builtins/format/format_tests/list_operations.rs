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
fn handles_core_list_operations_through_table_cases() {
    let cases = [
        (
            "list",
            list(&[Value::Integer(1), Value::Integer(2)]),
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
        ),
        (
            "list*",
            list_star(&[Value::Integer(1), Value::list(vec![Value::Integer(2)])]),
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
        ),
        (
            "make-list",
            make_list(&[Value::Integer(2)]),
            Value::list(vec![Value::Nil, Value::Nil]),
        ),
        (
            "values-list",
            values_list(&[Value::list(vec![Value::Integer(3)])]),
            Value::values(vec![Value::Integer(3)]),
        ),
    ];
    for (name, result, expected) in cases {
        let actual = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(actual.to_string(), expected.to_string());
    }
    assert!(list_star(&[]).is_err());
    assert!(make_list(&[Value::Integer(1), Value::keyword("unknown"), Value::Nil]).is_err());
    assert!(values_list(&[Value::Integer(1)]).is_err());
    assert!(make_list(&[]).is_err());
    assert!(make_list(&[Value::Integer(1), Value::keyword("initial-element")]).is_err());
    assert!(make_list(&[Value::string("not size")]).is_err());
    assert_value(
        make_list(&[
            Value::Integer(2),
            Value::keyword("initial-element"),
            Value::Integer(9),
        ]),
        Value::list(vec![Value::Integer(9), Value::Integer(9)]),
    );
    assert_value(list_length(&[Value::Nil]), Value::Integer(0));
    assert!(list_length(&[Value::Integer(1)]).is_err());
}

#[test]
fn handles_cons_property_and_list_access_operations() {
    let Ok(pair) = cons(&[Value::Integer(1), Value::list(vec![Value::Integer(2)])]) else {
        panic!("cons test input must be valid");
    };
    assert_value(car(std::slice::from_ref(&pair)), Value::Integer(1));
    assert_value(
        cdr(std::slice::from_ref(&pair)),
        Value::list(vec![Value::Integer(2)]),
    );
    assert_value(nth(&[Value::Integer(0), pair]), Value::Integer(1));
    assert_value(
        list_length(&[Value::list(vec![Value::Nil, Value::Nil])]),
        Value::Integer(2),
    );
    assert_value(
        acons(&[Value::symbol("key"), Value::Integer(3), Value::Nil]),
        Value::list(vec![Value::dotted_list(
            vec![Value::symbol("KEY")],
            Value::Integer(3),
        )]),
    );
    assert!(car(&[Value::Integer(1)]).is_err());
    assert_value(
        nth(&[Value::Integer(4), Value::list(vec![Value::Nil])]),
        Value::Nil,
    );

    let dotted = Value::dotted_list(
        vec![Value::Integer(2), Value::Integer(3)],
        Value::Integer(4),
    );
    assert_value(
        list_star(&[Value::Integer(1), dotted.clone()]),
        Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
            Value::Integer(4),
        ),
    );
    assert_value(
        nthcdr(&[Value::Integer(1), dotted.clone()]),
        Value::dotted_list(vec![Value::Integer(3)], Value::Integer(4)),
    );
    assert_value(
        nthcdr(&[Value::Integer(2), dotted.clone()]),
        Value::Integer(4),
    );
    assert!(nthcdr(&[Value::Integer(3), dotted.clone()]).is_err());
    assert_value(
        cdr(std::slice::from_ref(&dotted)),
        Value::dotted_list(vec![Value::Integer(3)], Value::Integer(4)),
    );
    assert_value(
        cons(&[Value::Integer(1), dotted.clone()]),
        Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
            Value::Integer(4),
        ),
    );
    assert_value(
        append(&[Value::list(vec![Value::Integer(1)]), dotted]),
        Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
            Value::Integer(4),
        ),
    );
    assert!(append(&[Value::Integer(1), Value::Nil]).is_err());
    assert_value(nthcdr(&[Value::Integer(0), Value::Nil]), Value::Nil);
    assert!(nthcdr(&[Value::Integer(0), Value::Integer(1)]).is_err());
    assert_value(
        pairlis(&[
            Value::list(vec![Value::symbol("a")]),
            Value::list(vec![Value::Integer(7)]),
        ]),
        Value::list(vec![Value::dotted_list(
            vec![Value::symbol("A")],
            Value::Integer(7),
        )]),
    );
    assert!(pairlis(&[Value::Nil]).is_err());
    assert!(pairlis(&[Value::list(vec![Value::Nil]), Value::Nil, Value::Integer(1)]).is_err());
    assert!(pairlis(&[Value::list(vec![Value::Nil]), Value::list(vec![])]).is_err());
}

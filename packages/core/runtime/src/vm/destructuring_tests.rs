use ncl_compiler::DestructurePattern;
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::destructuring::{destructure_dotted_parts, destructure_value};

#[test]
fn destructure_dotted_parts_normalizes_list_shapes() {
    let nested = Value::dotted_list(
        vec![Value::Integer(1)],
        Value::dotted_list(vec![Value::Integer(2)], Value::Integer(3)),
    );
    let cases = [
        (Value::Nil, vec![], Value::Nil),
        (
            Value::list(vec![Value::Integer(1)]),
            vec![Value::Integer(1)],
            Value::Nil,
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Nil),
            vec![Value::Integer(1)],
            Value::Nil,
        ),
        (
            Value::dotted_list(
                vec![Value::Integer(1)],
                Value::list(vec![Value::Integer(2)]),
            ),
            vec![Value::Integer(1), Value::Integer(2)],
            Value::Nil,
        ),
        (
            nested,
            vec![Value::Integer(1), Value::Integer(2)],
            Value::Integer(3),
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            vec![Value::Integer(1)],
            Value::Integer(2),
        ),
    ];

    for (value, expected_items, expected_tail) in cases {
        let Some((items, tail)) = destructure_dotted_parts(&value) else {
            panic!("a list-shaped value must be decomposed");
        };
        assert_eq!(
            items.iter().map(Value::to_string).collect::<Vec<_>>(),
            expected_items
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(tail.to_string(), expected_tail.to_string());
    }

    assert!(destructure_dotted_parts(&Value::Integer(1)).is_none());
}

#[test]
fn rejects_invalid_destructure_shapes() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);

    let cases = [
        (
            DestructurePattern::List(vec![]),
            Value::Integer(1),
            "destructuring-bind pattern requires a proper list",
        ),
        (
            DestructurePattern::List(vec![DestructurePattern::Name("x".to_string())]),
            Value::Nil,
            "destructuring-bind pattern has the wrong number of elements",
        ),
        (
            DestructurePattern::Dotted {
                items: vec![DestructurePattern::Name("x".to_string())],
                tail: Box::new(DestructurePattern::Name("rest".to_string())),
            },
            Value::Nil,
            "destructuring-bind pattern has too few elements",
        ),
    ];

    for (pattern, value, message) in cases {
        let result = destructure_value(&pattern, value, &runtime, &environment, span);
        assert!(
            matches!(result, Err(RuntimeError::InvalidForm { message: actual, .. }) if actual == message)
        );
    }
}

#[test]
fn binds_nested_destructure_patterns() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let cases = [
        (
            DestructurePattern::Name("value".to_string()),
            Value::Integer(1),
            vec![("value", "1")],
        ),
        (
            DestructurePattern::List(vec![
                DestructurePattern::Name("first".to_string()),
                DestructurePattern::Name("second".to_string()),
            ]),
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            vec![("first", "1"), ("second", "2")],
        ),
        (
            DestructurePattern::Dotted {
                items: vec![DestructurePattern::Name("first".to_string())],
                tail: Box::new(DestructurePattern::Name("rest".to_string())),
            },
            Value::list(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ]),
            vec![("first", "1"), ("rest", "(2 3)")],
        ),
    ];

    for (pattern, value, expected_bindings) in cases {
        assert!(destructure_value(&pattern, value, &runtime, &environment, span).is_ok());
        for (name, expected) in expected_bindings {
            let Some(actual) = environment.lookup(name) else {
                panic!("binding {name} was not created");
            };
            assert_eq!(actual.to_string(), expected);
        }
    }
}

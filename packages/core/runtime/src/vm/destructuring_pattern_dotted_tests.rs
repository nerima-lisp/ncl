use ncl_compiler::DestructurePattern;
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::destructuring::destructure_value;

#[test]
fn dotted_pattern_rejects_a_non_list_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let pattern = DestructurePattern::Dotted {
        items: vec![DestructurePattern::Name("first".to_string())],
        tail: Box::new(DestructurePattern::Name("rest".to_string())),
    };

    let result = destructure_value(&pattern, Value::Integer(1), &runtime, &environment, span);

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "destructuring-bind pattern requires a list"
    ));
}

#[test]
fn dotted_pattern_binds_leftover_items_and_a_genuine_dotted_tail() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let pattern = DestructurePattern::Dotted {
        items: vec![DestructurePattern::Name("first".to_string())],
        tail: Box::new(DestructurePattern::Name("rest".to_string())),
    };
    let value = Value::dotted_list(
        vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
        Value::Integer(4),
    );

    let result = destructure_value(&pattern, value, &runtime, &environment, span);

    assert!(result.is_ok());
    assert!(matches!(
        environment.lookup("first"),
        Some(Value::Integer(1))
    ));
    let Some(rest) = environment.lookup("rest") else {
        panic!("the dotted tail pattern must bind a value");
    };
    assert_eq!(rest.to_string(), "(2 3 . 4)");
}

#[test]
fn dotted_pattern_binds_an_empty_tail_when_nothing_remains() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let pattern = DestructurePattern::Dotted {
        items: vec![DestructurePattern::Name("first".to_string())],
        tail: Box::new(DestructurePattern::Name("rest".to_string())),
    };
    let value = Value::dotted_list(vec![Value::Integer(1)], Value::Nil);

    let result = destructure_value(&pattern, value, &runtime, &environment, span);

    assert!(result.is_ok());
    assert!(matches!(environment.lookup("rest"), Some(Value::Nil)));
}

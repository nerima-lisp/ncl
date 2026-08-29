use super::{destructure_specification, empty_lambda_list, empty_program};
use crate::{Environment, Runtime, RuntimeError, Value};
use ncl_compiler::{DestructurePattern, DestructureSpec};
use ncl_syntax::Span;

#[test]
fn rejects_a_non_list_value_for_a_lambda_list_specification() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list
        .required
        .push(DestructurePattern::Name("x".to_string()));

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::Integer(1),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "destructuring-bind value must be a proper list"
    ));
}

#[test]
fn rejects_too_few_required_arguments() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list
        .required
        .push(DestructurePattern::Name("x".to_string()));
    lambda_list
        .required
        .push(DestructurePattern::Name("y".to_string()));

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![Value::Integer(1)]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::Arity { expected, actual, .. })
            if expected == "at least 2" && actual == 1
    ));
}

#[test]
fn rejects_too_many_arguments_without_a_rest_or_keyword_section() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list
        .required
        .push(DestructurePattern::Name("x".to_string()));

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![Value::Integer(1), Value::Integer(2)]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::Arity { expected, actual, .. })
            if expected == "at most 1" && actual == 2
    ));
}

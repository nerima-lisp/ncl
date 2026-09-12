use super::{constant_program, destructure_specification, empty_lambda_list, empty_program};
use crate::{Environment, Runtime, RuntimeError, Value};
use ncl_compiler::{
    DestructureKeywordParameter, DestructureOptionalParameter, DestructurePattern, DestructureSpec,
};
use ncl_syntax::Span;

#[test]
fn rejects_an_out_of_range_optional_default_function() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.optional.push(DestructureOptionalParameter {
        pattern: DestructurePattern::Name("y".to_string()),
        default_function: 0,
        supplied_p: None,
    });

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::Nil,
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "compiled destructuring optional default is out of range"
    ));
}

#[test]
fn runs_the_optional_default_and_binds_supplied_p_when_omitted() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.optional.push(DestructureOptionalParameter {
        pattern: DestructurePattern::Name("y".to_string()),
        default_function: 0,
        supplied_p: Some("y-supplied".to_string()),
    });

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::Nil,
        &runtime,
        &constant_program(42),
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(
        environment.lookup("y").map(|v| v.to_string()).as_deref(),
        Some("42")
    );
    assert!(matches!(environment.lookup("y-supplied"), Some(Value::Nil)));
}

#[test]
fn rejects_keyword_arguments_that_are_not_supplied_in_pairs() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![Value::Keyword("FOO".into())]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "keyword arguments must be supplied in pairs"
    ));
}

#[test]
fn rejects_a_non_keyword_argument_name() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![Value::symbol("FOO"), Value::Integer(1)]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "keyword argument name must be a keyword"
    ));
}

#[test]
fn accepts_unknown_keywords_when_allow_other_keys_is_supplied_truthy() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![
            Value::Keyword("ALLOW-OTHER-KEYS".into()),
            Value::Boolean(true),
            Value::Keyword("UNDECLARED".into()),
            Value::Integer(1),
        ]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(result.is_ok());
}

mod tail;

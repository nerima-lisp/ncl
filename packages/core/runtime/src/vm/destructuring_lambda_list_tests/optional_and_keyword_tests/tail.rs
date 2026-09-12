use super::*;

#[test]
fn rejects_an_unknown_keyword_when_other_keys_are_not_allowed() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![Value::Keyword("UNDECLARED".into()), Value::Integer(1)]),
        &runtime,
        &empty_program(),
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "unknown keyword :UNDECLARED"
    ));
}

#[test]
fn rejects_an_out_of_range_keyword_default_function() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;
    lambda_list.keywords.push(DestructureKeywordParameter {
        keyword_name: "BAR".to_string(),
        pattern: DestructurePattern::Name("bar".to_string()),
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
            if message == "compiled destructuring keyword default is out of range"
    ));
}

#[test]
fn runs_the_keyword_default_when_the_keyword_is_omitted() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.has_keyword_section = true;
    lambda_list.keywords.push(DestructureKeywordParameter {
        keyword_name: "BAR".to_string(),
        pattern: DestructurePattern::Name("bar".to_string()),
        default_function: 0,
        supplied_p: None,
    });

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::Nil,
        &runtime,
        &constant_program(7),
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert!(matches!(environment.lookup("bar"), Some(Value::Integer(7))));
}

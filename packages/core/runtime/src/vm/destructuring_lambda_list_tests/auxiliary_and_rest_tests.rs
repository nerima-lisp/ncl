use super::{constant_program, destructure_specification, empty_lambda_list, empty_program};
use crate::{Environment, Runtime, RuntimeError, Value};
use ncl_compiler::{DestructureAuxiliaryParameter, DestructurePattern, DestructureSpec};
use ncl_syntax::Span;

#[test]
fn rejects_an_out_of_range_auxiliary_default_function() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list.auxiliary.push(DestructureAuxiliaryParameter {
        name: "aux".to_string(),
        default_function: 0,
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
            if message == "compiled destructuring auxiliary default is out of range"
    ));
}

#[test]
fn binds_required_rest_and_auxiliary_parameters_together() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut lambda_list = empty_lambda_list();
    lambda_list
        .required
        .push(DestructurePattern::Name("x".to_string()));
    lambda_list.rest = Some("rest".to_string());
    lambda_list.auxiliary.push(DestructureAuxiliaryParameter {
        name: "aux".to_string(),
        default_function: 0,
    });

    let result = destructure_specification(
        &DestructureSpec::LambdaList(lambda_list),
        Value::list(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ]),
        &runtime,
        &constant_program(5),
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert!(matches!(environment.lookup("x"), Some(Value::Integer(1))));
    assert_eq!(
        environment.lookup("rest").map(|v| v.to_string()).as_deref(),
        Some("(2 3)")
    );
    assert!(matches!(environment.lookup("aux"), Some(Value::Integer(5))));
}

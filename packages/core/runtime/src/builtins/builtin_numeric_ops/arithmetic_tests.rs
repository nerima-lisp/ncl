use super::arithmetic::{add, decrement, divide, increment, multiply, subtract};
use super::{RuntimeError, Value};

#[test]
fn handles_unary_and_zero_arity_arithmetic() {
    assert_eq!(numeric_result(increment(&[Value::Integer(4)])), "5");
    assert_eq!(numeric_result(decrement(&[Value::Integer(4)])), "3");
    assert_eq!(numeric_result(subtract(&[Value::Integer(4)])), "-4");
    assert_eq!(numeric_result(add(&[])), "0");
    assert_eq!(numeric_result(multiply(&[])), "1");
}

#[test]
fn rejects_invalid_arithmetic_arguments() {
    assert!(increment(&[]).is_err());
    assert!(subtract(&[]).is_err());
    assert!(divide(&[]).is_err());
}

#[test]
fn promotes_to_float_when_any_argument_is_float() {
    assert_eq!(
        numeric_result(add(&[Value::Integer(1), Value::Float(2.5)])),
        "3.5"
    );
    assert_eq!(
        numeric_result(subtract(&[Value::Integer(5), Value::Float(1.5)])),
        "3.5"
    );
    assert_eq!(
        numeric_result(multiply(&[Value::Integer(2), Value::Float(1.5)])),
        "3.0"
    );
}

#[test]
fn divides_single_float_argument_as_reciprocal() {
    assert_eq!(numeric_result(divide(&[Value::Float(2.0)])), "0.5");
    assert!(matches!(
        divide(&[Value::Float(0.0)]),
        Err(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn divides_float_arguments_and_rejects_float_division_by_zero() {
    assert_eq!(
        numeric_result(divide(&[Value::Float(6.0), Value::Float(2.0)])),
        "3.0"
    );
    assert!(matches!(
        divide(&[Value::Float(1.0), Value::Float(0.0)]),
        Err(RuntimeError::DivisionByZero)
    ));
}

fn numeric_result(result: Result<Value, RuntimeError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => panic!("unexpected numeric error: {error}"),
    }
}

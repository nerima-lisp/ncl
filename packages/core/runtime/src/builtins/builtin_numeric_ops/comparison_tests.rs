use std::cmp::Ordering;

use super::*;

#[test]
fn compares_and_bounds_numbers() {
    assert_eq!(
        numeric_result(compare_numbers(
            "<=",
            &[Value::Integer(1), Value::Integer(1)],
            |o| o != Ordering::Greater
        )),
        "T"
    );
    assert_eq!(
        numeric_result(compare_numbers(
            ">",
            &[Value::Integer(3), Value::Integer(2)],
            |o| o == Ordering::Greater
        )),
        "T"
    );
    assert_eq!(numeric_result(absolute(&[Value::Integer(-4)])), "4");
    assert_eq!(
        numeric_result(minimum(&[Value::Integer(3), Value::Integer(1)])),
        "1"
    );
    assert_eq!(
        numeric_result(maximum(&[Value::Integer(3), Value::Integer(1)])),
        "3"
    );
}

#[test]
fn rejects_invalid_comparison_arguments() {
    assert!(compare_numbers("<", &[], |_| true).is_err());
    assert!(absolute(&[Value::Nil]).is_err());
    assert!(minimum(&[]).is_err());
}

#[test]
fn public_comparison_predicates_delegate_correctly() {
    assert_eq!(
        numeric_result(greater_than(&[Value::Integer(3), Value::Integer(2)])),
        "T"
    );
    assert_eq!(
        numeric_result(greater_than(&[Value::Integer(2), Value::Integer(3)])),
        "NIL"
    );
    assert_eq!(
        numeric_result(less_equal(&[Value::Integer(1), Value::Integer(1)])),
        "T"
    );
    assert_eq!(
        numeric_result(less_equal(&[Value::Integer(2), Value::Integer(1)])),
        "NIL"
    );
    assert_eq!(
        numeric_result(greater_equal(&[Value::Integer(1), Value::Integer(1)])),
        "T"
    );
    assert_eq!(
        numeric_result(greater_equal(&[Value::Integer(1), Value::Integer(2)])),
        "NIL"
    );
}

#[test]
fn absolute_handles_rational_and_float_values() {
    let negative_half =
        Value::rational(-1, 2).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(numeric_result(absolute(&[negative_half])), "1/2");
    assert_eq!(numeric_result(absolute(&[Value::Float(-2.5)])), "2.5");
}

#[test]
fn absolute_returns_complex_magnitude() {
    let value = Value::complex(Value::Integer(3), Value::Integer(4));
    assert_eq!(numeric_result(absolute(&[value])), "5.0");
}

#[test]
fn numeric_equal_compares_complex_components() {
    let left = Value::complex(Value::Integer(2), Value::Integer(3));
    let right = Value::complex(Value::Integer(2), Value::Integer(3));
    let different = Value::complex(Value::Integer(2), Value::Integer(4));
    assert_eq!(numeric_result(numeric_equal(&[left, right])), "T");
    assert_eq!(
        numeric_result(numeric_equal(&[
            different,
            Value::complex(Value::Integer(2), Value::Integer(3))
        ])),
        "NIL"
    );
    assert_eq!(
        numeric_result(numeric_equal(&[
            Value::complex(Value::Integer(1), Value::Integer(0)),
            Value::Integer(1)
        ])),
        "T"
    );
}

fn numeric_result(result: Result<Value, RuntimeError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => panic!("unexpected numeric error: {error}"),
    }
}

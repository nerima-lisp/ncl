use super::*;

fn ok_string(result: Result<Value, RuntimeError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

#[test]
fn computes_gcd_lcm_and_shifts() {
    assert_eq!(
        ok_string(greatest_common_divisor(&[
            Value::Integer(12),
            Value::Integer(8)
        ])),
        "4"
    );
    assert_eq!(
        ok_string(least_common_multiple(&[
            Value::Integer(4),
            Value::Integer(6)
        ])),
        "12"
    );
    assert_eq!(
        ok_string(arithmetic_shift(&[Value::Integer(1), Value::Integer(3)])),
        "8"
    );
    assert_eq!(ok_string(numerator(&[Value::Integer(5)])), "5");
    assert_eq!(ok_string(denominator(&[Value::Integer(5)])), "1");
}

#[test]
fn rejects_invalid_integer_operation_arguments() {
    assert!(numerator(&[Value::Nil]).is_err());
    assert!(arithmetic_shift(&[Value::Integer(1)]).is_err());
    assert!(denominator(&[Value::Nil]).is_err());
}

#[test]
fn lcm_short_circuits_to_zero_when_any_argument_is_zero() {
    assert_eq!(
        ok_string(least_common_multiple(&[
            Value::Integer(0),
            Value::Integer(6)
        ])),
        "0"
    );
    assert_eq!(
        ok_string(least_common_multiple(&[
            Value::Integer(4),
            Value::Integer(0)
        ])),
        "0"
    );
}

#[test]
fn arithmetic_shift_handles_boundaries() {
    assert_eq!(
        ok_string(arithmetic_shift(&[Value::Integer(0), Value::Integer(64)])),
        "0"
    );
    assert_eq!(
        ok_string(arithmetic_shift(&[Value::Integer(1), Value::Integer(64)])),
        "18446744073709551616"
    );
    assert_eq!(
        ok_string(arithmetic_shift(&[
            Value::Integer(5),
            Value::Integer(i64::MIN)
        ])),
        "0"
    );
    assert_eq!(
        ok_string(arithmetic_shift(&[
            Value::Integer(-5),
            Value::Integer(i64::MIN)
        ])),
        "-1"
    );
    assert_eq!(
        ok_string(arithmetic_shift(&[
            Value::Integer(-5),
            Value::Integer(-100)
        ])),
        "-1"
    );
}

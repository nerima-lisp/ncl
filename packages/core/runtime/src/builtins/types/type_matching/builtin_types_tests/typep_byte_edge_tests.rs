use crate::builtins::types::subtype_entry::typep_value;
use crate::{RuntimeError, Value};

use super::support::compound;

fn matches(result: Result<bool, RuntimeError>, context: &str) -> bool {
    result.unwrap_or_else(|error| panic!("{context}: {error}"))
}

#[test]
fn typep_unsigned_byte_spec_handles_sign_wildcards_and_large_sizes() {
    let non_integer_value = typep_value(
        &Value::String("s".into()),
        &compound("unsigned-byte", vec![Value::Integer(8)]),
    );
    assert!(!matches(
        non_integer_value,
        "a non-integer value simply fails to match"
    ));

    let negative_value = typep_value(
        &Value::Integer(-1),
        &compound("unsigned-byte", vec![Value::Integer(8)]),
    );
    assert!(!matches(
        negative_value,
        "a negative value is never an unsigned byte"
    ));

    let wildcard_size = typep_value(
        &Value::Integer(200),
        &compound("unsigned-byte", vec![Value::symbol("*")]),
    );
    assert!(matches(
        wildcard_size,
        "a wildcard size accepts any non-negative value"
    ));

    let huge_size = typep_value(
        &Value::Integer(200),
        &compound("unsigned-byte", vec![Value::Integer(100)]),
    );
    assert!(matches(
        huge_size,
        "a size at or above 63 bits always matches non-negative values"
    ));
}

#[test]
fn typep_signed_byte_spec_handles_call_site_errors_wildcards_and_zero_size() {
    let negative_size_error = typep_value(
        &Value::Integer(1),
        &compound("signed-byte", vec![Value::Integer(-1)]),
    );
    assert!(
        negative_size_error.is_err(),
        "a negative SIGNED-BYTE size must propagate an error"
    );

    let non_integer_value = typep_value(
        &Value::String("s".into()),
        &compound("signed-byte", vec![Value::Integer(8)]),
    );
    assert!(!matches(
        non_integer_value,
        "a non-integer value simply fails to match"
    ));

    let wildcard_size = typep_value(
        &Value::Integer(-5),
        &compound("signed-byte", vec![Value::symbol("*")]),
    );
    assert!(matches(
        wildcard_size,
        "a wildcard size accepts any integer"
    ));

    let zero_size = typep_value(
        &Value::Integer(0),
        &compound("signed-byte", vec![Value::Integer(0)]),
    );
    assert!(!matches(
        zero_size,
        "a zero-width signed byte matches nothing"
    ));
}

#[test]
fn typep_byte_type_size_reports_arity_and_type_errors() {
    let too_many_args = typep_value(
        &Value::Integer(1),
        &compound("signed-byte", vec![Value::Integer(1), Value::Integer(2)]),
    );
    assert!(
        too_many_args.is_err(),
        "byte specs take at most one size argument"
    );

    let non_integer_size = typep_value(
        &Value::Integer(1),
        &compound("unsigned-byte", vec![Value::symbol("not-a-size")]),
    );
    assert!(
        non_integer_size.is_err(),
        "a non-integer, non-wildcard size is invalid"
    );

    let unbounded = typep_value(&Value::Integer(1), &compound("signed-byte", Vec::new()));
    assert!(matches(
        unbounded,
        "omitting the size means an unbounded byte spec"
    ));
}

use crate::builtins::types::subtype_entry::typep_value;
use crate::{RuntimeError, Value};

use super::support::compound;

fn matches(result: Result<bool, RuntimeError>, context: &str) -> bool {
    result.unwrap_or_else(|error| panic!("{context}: {error}"))
}

#[test]
fn typep_integer_spec_reports_bound_errors_and_type_mismatches() {
    let bad_lower = typep_value(
        &Value::Integer(5),
        &compound("integer", vec![Value::String("x".into())]),
    );
    assert!(bad_lower.is_err(), "a non-integer lower bound is invalid");

    let bad_upper = typep_value(
        &Value::Integer(5),
        &compound(
            "integer",
            vec![Value::Integer(0), Value::String("y".into())],
        ),
    );
    assert!(bad_upper.is_err(), "a non-integer upper bound is invalid");

    let wildcard_lower = typep_value(
        &Value::Integer(5),
        &compound("integer", vec![Value::symbol("*"), Value::Integer(10)]),
    );
    assert!(matches(wildcard_lower, "a wildcard lower bound is valid"));

    let non_integer_value = typep_value(
        &Value::String("s".into()),
        &compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
    );
    assert!(!matches(
        non_integer_value,
        "a non-integer value simply fails to match"
    ));
}

#[test]
fn typep_mod_spec_reports_arity_errors_and_type_mismatches() {
    let no_args = typep_value(&Value::Integer(1), &compound("mod", Vec::new()));
    assert!(no_args.is_err(), "MOD requires exactly one argument");

    let non_integer_modulus = typep_value(
        &Value::Integer(1),
        &compound("mod", vec![Value::symbol("x")]),
    );
    assert!(
        non_integer_modulus.is_err(),
        "MOD's modulus must be an integer"
    );

    let non_integer_value = typep_value(
        &Value::String("s".into()),
        &compound("mod", vec![Value::Integer(4)]),
    );
    assert!(!matches(
        non_integer_value,
        "a non-integer value simply fails to match"
    ));
}

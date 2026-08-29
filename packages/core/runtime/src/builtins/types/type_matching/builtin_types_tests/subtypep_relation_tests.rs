use crate::builtins::types::subtype_entry::subtypep_value;
use crate::{Environment, Value};

use super::support::compound;

#[test]
fn subtypep_table_covers_integer_boundaries_and_logical_relations() {
    let environment = Environment::new();
    let cases = [
        (
            compound("integer", vec![]),
            compound("integer", vec![]),
            true,
        ),
        (
            compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
            compound("integer", vec![Value::Integer(-1), Value::Integer(10)]),
            true,
        ),
        (
            compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
            compound("integer", vec![Value::Integer(1), Value::Integer(9)]),
            false,
        ),
        (
            compound(
                "or",
                vec![Value::symbol("integer"), Value::symbol("string")],
            ),
            Value::symbol("atom"),
            true,
        ),
        (
            Value::symbol("integer"),
            compound("or", vec![Value::symbol("number"), Value::symbol("string")]),
            true,
        ),
    ];
    for (subtype, supertype, expected) in cases {
        let result = match subtypep_value(&subtype, &supertype, &environment) {
            Ok(result) => result,
            Err(error) => panic!("valid subtype relation: {error}"),
        };
        let Value::Values(values) = result else {
            panic!("SUBTYPEP must return two values")
        };
        assert_eq!(
            values.as_ref()[0].is_truthy(),
            expected,
            "{subtype:?} <: {supertype:?}"
        );
    }
}

#[test]
fn subtypep_reports_known_and_unknown_relations() {
    let environment = Environment::new();
    let known = match subtypep_value(
        &Value::symbol("integer"),
        &Value::symbol("number"),
        &environment,
    ) {
        Ok(result) => result,
        Err(error) => panic!("known subtype designators: {error}"),
    };
    let Value::Values(values) = known else {
        panic!("SUBTYPEP returns two values");
    };
    assert!(matches!(
        values.as_ref().as_slice(),
        [Value::Boolean(true), Value::Boolean(true)]
    ));

    let unknown = subtypep_value(
        &Value::symbol("integer"),
        &Value::symbol("not-a-type"),
        &environment,
    );
    assert!(unknown.is_err());
}

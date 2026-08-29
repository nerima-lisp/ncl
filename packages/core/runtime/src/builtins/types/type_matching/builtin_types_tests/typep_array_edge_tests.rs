use crate::builtins::types::subtype_entry::typep_value;
use crate::{RuntimeError, Value};

use super::support::compound;

fn one_element_array() -> Value {
    Value::vector(vec![Value::Integer(1)])
}

fn matches(result: Result<bool, RuntimeError>, context: &str) -> bool {
    result.unwrap_or_else(|error| panic!("{context}: {error}"))
}

#[test]
fn typep_array_spec_reports_dimension_errors_and_mismatches() {
    let value = one_element_array();

    let non_array_value = typep_value(&Value::Integer(1), &compound("array", Vec::new()));
    assert!(!matches(
        non_array_value,
        "a non-array value simply fails to match"
    ));

    let nil_dimensions = typep_value(
        &value,
        &compound("array", vec![Value::symbol("*"), Value::Nil]),
    );
    assert!(
        !matches(nil_dimensions, "NIL dimensions require a zero-rank array"),
        "a one-element array is not zero-rank"
    );

    let negative_rank = typep_value(
        &value,
        &compound("array", vec![Value::symbol("*"), Value::Integer(-1)]),
    );
    assert!(negative_rank.is_err(), "a negative array rank is invalid");

    let rank_mismatch = typep_value(
        &value,
        &compound(
            "array",
            vec![
                Value::symbol("*"),
                Value::list(vec![Value::Integer(1), Value::Integer(1)]),
            ],
        ),
    );
    assert!(!matches(
        rank_mismatch,
        "a dimension list of the wrong length simply fails to match"
    ));

    let wildcard_dimension = typep_value(
        &value,
        &compound(
            "array",
            vec![Value::symbol("*"), Value::list(vec![Value::symbol("*")])],
        ),
    );
    assert!(matches(
        wildcard_dimension,
        "a wildcard dimension entry matches any size"
    ));

    let non_integer_dimension = typep_value(
        &value,
        &compound(
            "array",
            vec![
                Value::symbol("*"),
                Value::list(vec![Value::String("bad".into())]),
            ],
        ),
    );
    assert!(
        non_integer_dimension.is_err(),
        "a non-integer, non-wildcard dimension entry is invalid"
    );

    let negative_dimension = typep_value(
        &value,
        &compound(
            "array",
            vec![Value::symbol("*"), Value::list(vec![Value::Integer(-1)])],
        ),
    );
    assert!(
        negative_dimension.is_err(),
        "a negative dimension entry is invalid"
    );

    let non_list_dimensions = typep_value(
        &value,
        &compound(
            "array",
            vec![Value::symbol("*"), Value::String("nope".into())],
        ),
    );
    assert!(
        non_list_dimensions.is_err(),
        "a dimension spec that is neither NIL, an integer, nor a list is invalid"
    );

    let bad_element_type = typep_value(&value, &compound("array", vec![Value::Integer(5)]));
    assert!(
        bad_element_type.is_err(),
        "an invalid element type designator must propagate an error"
    );

    let no_constraints = typep_value(&value, &compound("array", Vec::new()));
    assert!(matches(
        no_constraints,
        "an ARRAY designator with no arguments matches any array"
    ));
}

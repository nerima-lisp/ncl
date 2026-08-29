use std::rc::Rc;

use crate::builtins::types::subtype_entry::subtypep_value;
use crate::builtins::types::subtype_validation::validate_subtype_designator;
use crate::{Environment, Value};

use super::support::compound;

#[test]
fn validate_subtype_designator_rejects_malformed_lists_directly() {
    let environment = Environment::new();

    // `Value::list` collapses an empty vector to NIL, so the "compound list
    // with no operator" branch can only be reached by constructing the
    // empty `Value::List` directly.
    let empty_list = Value::List(Rc::new(Vec::new()));
    assert!(validate_subtype_designator("subtypep", &empty_list, &environment).is_err());

    // A compound designator whose operator position is not a symbol.
    let numeric_operator = Value::List(Rc::new(vec![Value::Integer(1), Value::Integer(2)]));
    assert!(validate_subtype_designator("subtypep", &numeric_operator, &environment).is_err());
}

#[test]
fn subtypep_rejects_designators_that_pass_a_non_symbol_atom_directly() {
    let environment = Environment::new();
    let result = subtypep_value(&Value::Integer(5), &Value::symbol("t"), &environment);
    assert!(result.is_err(), "a bare integer is not a type designator");
}

#[test]
fn subtypep_rejects_nested_and_out_of_range_compound_arguments() {
    let environment = Environment::new();
    let invalid = [
        compound("or", vec![Value::symbol("not-a-type")]),
        compound("not", vec![Value::symbol("not-a-type")]),
        compound("integer", vec![Value::String("x".into())]),
        compound("mod", Vec::new()),
        compound("mod", vec![Value::Integer(1), Value::Integer(2)]),
        compound(
            "cons",
            vec![
                Value::symbol("integer"),
                Value::symbol("integer"),
                Value::symbol("integer"),
            ],
        ),
        compound("cons", vec![Value::symbol("not-a-type")]),
        compound("vector", vec![Value::symbol("not-a-type")]),
        compound(
            "vector",
            vec![Value::symbol("integer"), Value::symbol("not-a-size")],
        ),
        compound("simple-vector", vec![Value::Integer(1), Value::Integer(2)]),
        compound("simple-vector", vec![Value::symbol("not-a-size")]),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::Integer(1),
                Value::Integer(2),
            ],
        ),
        compound("array", vec![Value::symbol("not-a-type")]),
        compound("bogus-compound-operator", vec![Value::Integer(1)]),
    ];
    for designator in invalid {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(
            result.is_err(),
            "expected {designator:?} to be rejected as a type designator"
        );
    }
}

#[test]
fn subtypep_accepts_compound_designators_with_omitted_optional_arguments() {
    let environment = Environment::new();
    let valid = [
        compound("vector", Vec::new()),
        compound("vector", vec![Value::symbol("integer")]),
        compound("vector", vec![Value::symbol("*"), Value::Integer(3)]),
        compound("simple-vector", Vec::new()),
        compound("array", Vec::new()),
        compound("array", vec![Value::symbol("integer")]),
        compound("array", vec![Value::symbol("integer"), Value::Nil]),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::list(vec![Value::symbol("*"), Value::Integer(3)]),
            ],
        ),
    ];
    for designator in valid {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(
            result.is_ok(),
            "expected {designator:?} to be a valid type designator: {result:?}"
        );
    }
}

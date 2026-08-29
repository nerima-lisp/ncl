use std::rc::Rc;

use crate::Value;
use crate::builtins::types::subtype_entry::typep_value;
use crate::builtins::types::type_matching::dispatch::type_matches_designator;

use super::support::compound;

#[test]
fn typep_rejects_a_bare_non_symbol_atom_as_a_type_designator() {
    let result = typep_value(&Value::Nil, &Value::Integer(5));
    assert!(result.is_err(), "a bare integer is not a type designator");
}

#[test]
fn type_matches_designator_rejects_malformed_compound_lists_directly() {
    let empty_list = Value::List(Rc::new(Vec::new()));
    assert!(
        type_matches_designator("typep", &Value::Nil, &empty_list).is_err(),
        "a compound designator with no operator is invalid"
    );

    let numeric_operator = Value::List(Rc::new(vec![Value::Integer(1)]));
    assert!(
        type_matches_designator("typep", &Value::Nil, &numeric_operator).is_err(),
        "a compound designator whose operator is not a symbol is invalid"
    );
}

#[test]
fn typep_or_and_not_propagate_nested_designator_errors() {
    for operator in ["or", "and", "not"] {
        let result = typep_value(&Value::Nil, &compound(operator, vec![Value::Integer(5)]));
        assert!(
            result.is_err(),
            "{operator} must propagate an error from an invalid nested designator"
        );
    }
}

#[test]
fn typep_or_reports_false_when_every_branch_fails_to_match() {
    let result = typep_value(
        &Value::Integer(1),
        &compound(
            "or",
            vec![Value::symbol("string"), Value::symbol("character")],
        ),
    )
    .unwrap_or_else(|error| panic!("OR with only non-matching branches must not error: {error}"));
    assert!(!result);
}

#[test]
fn typep_eql_reports_an_arity_error_for_zero_or_extra_arguments() {
    let no_args = typep_value(&Value::Integer(1), &compound("eql", Vec::new()));
    assert!(no_args.is_err(), "EQL requires exactly one argument");

    let extra_args = typep_value(
        &Value::Integer(1),
        &compound("eql", vec![Value::Integer(1), Value::Integer(2)]),
    );
    assert!(extra_args.is_err(), "EQL rejects more than one argument");
}

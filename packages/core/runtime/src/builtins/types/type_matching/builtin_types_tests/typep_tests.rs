use crate::Value;
use crate::builtins::types::subtype_entry::typep_value;

use super::support::compound;

#[test]
fn typep_supports_logical_and_numeric_designators() {
    let integer = Value::Integer(7);
    let and_result = match typep_value(
        &integer,
        &compound(
            "and",
            vec![Value::symbol("number"), Value::symbol("integer")],
        ),
    ) {
        Ok(result) => result,
        Err(error) => panic!("valid AND type designator: {error}"),
    };
    assert!(and_result);
    let integer_result = match typep_value(
        &integer,
        &compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
    ) {
        Ok(result) => result,
        Err(error) => panic!("valid INTEGER type designator: {error}"),
    };
    assert!(integer_result);
    let mod_result = match typep_value(&integer, &compound("mod", vec![Value::Integer(4)])) {
        Ok(result) => result,
        Err(error) => panic!("valid MOD type designator: {error}"),
    };
    assert!(!mod_result);
}

#[test]
fn typep_classifies_atomic_values_from_table_cases() {
    let cases = [
        (Value::Nil, "null", true),
        (Value::Unbound, "unbound", true),
        (Value::Boolean(true), "boolean", true),
        (Value::Integer(7), "integer", true),
        (Value::Integer(7), "float", false),
        (Value::Float(2.5), "real", true),
        (Value::Character('x'), "character", true),
        (Value::symbol("answer"), "symbol", true),
        (Value::keyword("answer"), "keyword", true),
        (Value::list(vec![Value::Integer(1)]), "cons", true),
        (Value::vector(vec![Value::Integer(1)]), "vector", true),
    ];

    for (value, designator, expected) in cases {
        let actual = typep_value(&value, &Value::symbol(designator))
            .unwrap_or_else(|error| panic!("{designator} rejected for {value:?}: {error}"));
        assert_eq!(actual, expected, "{value:?} against {designator}");
    }
}

#[test]
fn typep_covers_atomic_designator_matrix() {
    let cases = [
        (Value::Nil, "list", true),
        (Value::Nil, "atom", true),
        (Value::list(vec![Value::Integer(1)]), "atom", false),
        (Value::String("text".into()), "string", true),
        (Value::String("text".into()), "sequence", true),
        (Value::String("text".into()), "vector", true),
        (Value::Float(1.0), "rational", false),
        (Value::Character('x'), "atom", true),
        (Value::Character('x'), "sequence", false),
        (
            Value::vector(vec![Value::Integer(0), Value::Integer(1)]),
            "bit-vector",
            true,
        ),
        (Value::vector(vec![Value::Integer(2)]), "bit-vector", false),
        (Value::Unbound, "unbound", true),
        (Value::Unbound, "values", false),
        (Value::Values(vec![Value::Nil].into()), "values", true),
        (Value::Values(vec![Value::Nil].into()), "atom", true),
    ];

    for (value, designator, expected) in cases {
        let actual = typep_value(&value, &Value::symbol(designator))
            .unwrap_or_else(|error| panic!("{designator} rejected for {value:?}: {error}"));
        assert_eq!(actual, expected, "{value:?} against {designator}");
    }
}

#[test]
fn typep_rejects_malformed_compound_designators() {
    let malformed = compound("not", Vec::new());
    let error = match typep_value(&Value::Nil, &malformed) {
        Ok(value) => panic!("NOT should reject zero arguments, got {value}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("expects between 1 and 1"));

    let dotted = Value::dotted_list(vec![Value::symbol("or")], Value::symbol("integer"));
    assert!(typep_value(&Value::Nil, &dotted).is_err());
}

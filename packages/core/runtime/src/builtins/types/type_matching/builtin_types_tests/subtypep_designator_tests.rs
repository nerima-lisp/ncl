use crate::builtins::types::subtype_entry::subtypep_value;
use crate::{Environment, Value};

use super::support::compound;

#[test]
fn subtypep_accepts_supported_compound_designators() {
    let environment = Environment::new();
    let designators = vec![
        compound(
            "or",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound(
            "and",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound("not", vec![Value::symbol("integer")]),
        compound("eql", vec![Value::Integer(1)]),
        compound("member", vec![Value::Integer(1), Value::Integer(2)]),
        compound("integer", vec![Value::Integer(0), Value::Integer(10)]),
        compound("mod", vec![Value::Integer(4)]),
        compound("signed-byte", vec![Value::Integer(8)]),
        compound("unsigned-byte", vec![Value::Integer(8)]),
        compound(
            "cons",
            vec![Value::symbol("integer"), Value::symbol("number")],
        ),
        compound("vector", vec![Value::symbol("integer"), Value::Integer(2)]),
        compound("simple-vector", vec![Value::Integer(2)]),
        compound("bit-vector", vec![Value::Integer(2)]),
        compound("simple-bit-vector", vec![Value::Integer(2)]),
        compound("array", vec![Value::symbol("integer"), Value::Integer(2)]),
        compound(
            "simple-array",
            vec![Value::symbol("integer"), Value::symbol("*")],
        ),
    ];

    for designator in designators {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(result.is_ok(), "valid designator rejected: {designator:?}");
    }
}

#[test]
fn subtypep_rejects_invalid_compound_designators() {
    let environment = Environment::new();
    let designators = vec![
        compound("not", Vec::new()),
        compound("eql", vec![Value::Integer(1), Value::Integer(2)]),
        compound("mod", vec![Value::Integer(-1)]),
        compound("mod", vec![Value::symbol("integer")]),
        compound("signed-byte", vec![Value::Integer(-1)]),
        compound("vector", vec![Value::symbol("integer"), Value::Integer(-1)]),
        compound("array", vec![Value::symbol("integer"), Value::Integer(-1)]),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::list(vec![Value::String("dimension".into())]),
            ],
        ),
        compound(
            "array",
            vec![
                Value::symbol("integer"),
                Value::list(vec![Value::Integer(-1)]),
            ],
        ),
        compound(
            "array",
            vec![Value::symbol("integer"), Value::symbol("invalid")],
        ),
        Value::dotted_list(vec![Value::symbol("or")], Value::symbol("integer")),
    ];

    for designator in designators {
        let result = subtypep_value(&designator, &Value::symbol("t"), &environment);
        assert!(
            result.is_err(),
            "invalid designator accepted: {designator:?}"
        );
    }
}

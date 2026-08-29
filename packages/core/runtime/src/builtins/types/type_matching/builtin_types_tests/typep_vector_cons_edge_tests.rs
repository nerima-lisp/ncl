use crate::Value;
use crate::builtins::types::subtype_entry::typep_value;

use super::support::compound;

#[test]
fn typep_cons_spec_reports_arity_element_and_shape_errors() {
    let pair = Value::list(vec![Value::Integer(1), Value::Integer(2)]);

    let too_many_args = typep_value(
        &pair,
        &compound(
            "cons",
            vec![
                Value::symbol("integer"),
                Value::symbol("integer"),
                Value::symbol("integer"),
            ],
        ),
    );
    assert!(too_many_args.is_err(), "CONS accepts at most two arguments");

    let invalid_first_slot_designator =
        typep_value(&pair, &compound("cons", vec![Value::Integer(5)]));
    assert!(
        invalid_first_slot_designator.is_err(),
        "an invalid car type designator errors"
    );

    let first_slot_mismatch = typep_value(&pair, &compound("cons", vec![Value::symbol("string")]))
        .unwrap_or_else(|error| panic!("a mismatched car type simply fails to match: {error}"));
    assert!(!first_slot_mismatch);

    let invalid_second_slot_designator = typep_value(
        &pair,
        &compound("cons", vec![Value::symbol("t"), Value::Integer(5)]),
    );
    assert!(
        invalid_second_slot_designator.is_err(),
        "an invalid cdr type designator errors"
    );

    let single_element_list = typep_value(
        &Value::list(vec![Value::Integer(1)]),
        &compound("cons", Vec::new()),
    )
    .unwrap_or_else(|error| panic!("a one-element list is a cons with a NIL tail: {error}"));
    assert!(single_element_list);
}

#[test]
fn typep_vector_family_specs_report_size_and_element_errors() {
    let vector = Value::vector(vec![Value::Integer(1)]);

    for operator in ["vector", "simple-vector", "bit-vector"] {
        let bad_size = typep_value(
            &vector,
            &compound(operator, vec![Value::String("bad".into())]),
        );
        assert!(bad_size.is_err(), "{operator} rejects a non-integer size");

        let non_vector_value = typep_value(&Value::Integer(1), &compound(operator, Vec::new()))
            .unwrap_or_else(|error| {
                panic!("{operator} against a non-vector must not error: {error}")
            });
        assert!(!non_vector_value);
    }

    let bad_vector_size_arity = typep_value(
        &vector,
        &compound("simple-vector", vec![Value::Integer(1), Value::Integer(2)]),
    );
    assert!(
        bad_vector_size_arity.is_err(),
        "SIMPLE-VECTOR accepts at most one size argument"
    );

    let bad_bit_vector_arity = typep_value(
        &vector,
        &compound("bit-vector", vec![Value::Integer(1), Value::Integer(2)]),
    );
    assert!(
        bad_bit_vector_arity.is_err(),
        "BIT-VECTOR accepts at most one size argument"
    );

    let bad_element_type = typep_value(&vector, &compound("vector", vec![Value::Integer(5)]));
    assert!(
        bad_element_type.is_err(),
        "an invalid element type designator must propagate an error"
    );

    let no_constraints =
        typep_value(&vector, &compound("vector", Vec::new())).unwrap_or_else(|error| {
            panic!("a VECTOR designator with no arguments matches any vector: {error}")
        });
    assert!(no_constraints);
}

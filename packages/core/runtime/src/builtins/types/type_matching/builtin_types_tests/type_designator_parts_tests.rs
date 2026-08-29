use std::rc::Rc;

use crate::Value;
use crate::builtins::types::type_designator_parts::{compound_type_parts, same_type_designator};

#[test]
fn compound_type_parts_declines_malformed_compound_lists() {
    let empty_list = Value::List(Rc::new(Vec::new()));
    assert!(compound_type_parts(&empty_list).is_none());

    let numeric_operator = Value::List(Rc::new(vec![Value::Integer(1)]));
    assert!(compound_type_parts(&numeric_operator).is_none());
}

#[test]
fn same_type_designator_compares_compound_designators_structurally() {
    let or_ab = Value::list(vec![
        Value::symbol("or"),
        Value::symbol("integer"),
        Value::symbol("string"),
    ]);
    let or_a = Value::list(vec![Value::symbol("or"), Value::symbol("integer")]);
    assert!(
        !same_type_designator(&or_ab, &or_a),
        "designators with a different argument count differ"
    );

    let left_bad_operator = Value::List(Rc::new(vec![Value::Integer(1), Value::symbol("x")]));
    let right_named = Value::list(vec![Value::symbol("or"), Value::symbol("x")]);
    assert!(
        !same_type_designator(&left_bad_operator, &right_named),
        "a left operand whose operator is not a symbol cannot match"
    );
    assert!(
        !same_type_designator(&right_named, &left_bad_operator),
        "a right operand whose operator is not a symbol cannot match"
    );

    let and_x = Value::list(vec![Value::symbol("and"), Value::symbol("x")]);
    assert!(
        !same_type_designator(&or_a, &and_x),
        "OR and AND designators are never the same type designator"
    );

    let member_one_two = Value::list(vec![
        Value::symbol("member"),
        Value::Integer(1),
        Value::Integer(2),
    ]);
    let member_one_three = Value::list(vec![
        Value::symbol("member"),
        Value::Integer(1),
        Value::Integer(3),
    ]);
    assert!(
        !same_type_designator(&member_one_two, &member_one_three),
        "MEMBER designators compare their candidates with EQL, not recursively"
    );
    assert!(same_type_designator(
        &member_one_two,
        &member_one_two.clone()
    ));

    assert!(
        !same_type_designator(&Value::symbol("integer"), &Value::Integer(5)),
        "a named type designator never equals a non-designator atom"
    );
}

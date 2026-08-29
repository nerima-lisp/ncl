use std::rc::Rc;

use crate::RuntimeError;
use crate::builtins::*;

#[test]
fn sequence_primitives_reject_bad_inputs_and_handle_zero_counts() {
    assert!(last(&[]).is_err());
    assert!(last(&[Value::Integer(1)]).is_err());
    assert!(matches!(
        last(&[Value::list(vec![Value::Integer(1)]), Value::Integer(0)]),
        Ok(Value::Nil)
    ));
    assert!(butlast(&[]).is_err());
    assert!(butlast(&[Value::Integer(1)]).is_err());
    assert!(copy_list(&[Value::Integer(1)]).is_err());
    assert!(copy_alist(&[Value::Integer(1)]).is_err());
    assert!(copy_alist(&[Value::list(vec![Value::Integer(1)])]).is_err());
}

#[test]
fn sequence_copy_primitives_cover_table_driven_success_cases() -> Result<(), RuntimeError> {
    type Primitive = fn(&[Value]) -> Result<Value, RuntimeError>;

    let list = Value::list(vec![Value::Integer(1), Value::Integer(2)]);
    let alist = Value::list(vec![Value::list(vec![
        Value::keyword("key"),
        Value::Integer(1),
    ])]);
    let cases: [(Primitive, Value, &str); 3] = [
        (last, list.clone(), "(2)"),
        (butlast, list.clone(), "(1)"),
        (copy_list, list, "(1 2)"),
    ];

    for (primitive, input, expected) in cases {
        assert_eq!(primitive(&[input])?.to_string(), expected);
    }
    assert_eq!(copy_alist(&[alist])?.to_string(), "((:KEY 1))");
    Ok(())
}

#[test]
fn data_helpers_cover_successful_table_cases() -> Result<(), RuntimeError> {
    let tree = Value::dotted_list(
        vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])],
        Value::Integer(3),
    );
    assert_eq!(copy_tree(&[tree])?.to_string(), "((1 2) . 3)");
    let cases = [
        (
            Value::Vector(Rc::new(vec![Value::Integer(1), Value::Integer(2)])),
            vec![2],
        ),
        (
            Value::Array {
                dimensions: Rc::new(vec![1, 2]),
                elements: Rc::new(vec![Value::Integer(1), Value::Integer(2)]),
            },
            vec![1, 2],
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(dimensions_for_array(&value), Some(expected));
        assert!(array_elements(&value).is_some());
    }
    assert!(sequence_items(&Value::list(vec![Value::Integer(1)])).is_some());
    assert!(sequence_items(&Value::Integer(1)).is_none());
    Ok(())
}

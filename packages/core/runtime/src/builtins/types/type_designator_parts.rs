#![allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn compound_type_parts(value: &Value) -> Option<(String, Vec<Value>)> {
    let items = value.list_items()?;
    let operator = type_designator_name("subtypep", items.first()?).ok()?;
    Some((operator, items[1..].to_vec()))
}

pub(crate) fn atomic_type_name(value: &Value) -> Option<String> {
    if matches!(value, Value::Cons(_)) {
        None
    } else {
        type_designator_name("subtypep", value).ok()
    }
}

pub(crate) fn same_type_designator(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Cons(_), Value::Cons(_)) => {
            let (Some(left), Some(right)) = (left.list_items(), right.list_items()) else {
                return false;
            };
            if left.len() != right.len() {
                return false;
            }
            let Some(left_operator) = left
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            let Some(right_operator) = right
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            if left_operator != right_operator {
                return false;
            }
            left.iter()
                .zip(right.iter())
                .enumerate()
                .all(|(index, (left, right))| {
                    if index == 0 {
                        true
                    } else if matches!(left_operator.as_str(), "MEMBER" | "EQL") {
                        eql_value(left, right)
                    } else {
                        same_type_designator(left, right)
                    }
                })
        }
        (Value::Cons(_), _) | (_, Value::Cons(_)) => false,
        _ => match (
            type_designator_name("subtypep", left).ok(),
            type_designator_name("subtypep", right).ok(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => eql_value(left, right),
            _ => false,
        },
    }
}

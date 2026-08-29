use std::cmp::Ordering;

use super::{exact, integer_from_usize, string_designator};
use crate::{RuntimeError, Value};

pub fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

pub fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

pub fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

pub fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

pub fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

pub fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

pub fn compare_strings(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (index, ordering) = string_order(&left, &right, ignore_case);
    if comparison(ordering) {
        integer_from_usize(function, index)
    } else {
        Ok(Value::Nil)
    }
}

pub fn string_equality(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (_, ordering) = string_order(&left, &right, ignore_case);
    Ok(Value::boolean(ordering == Ordering::Equal))
}

pub fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    for (index, (left, right)) in left.iter().zip(&right).enumerate() {
        let left = if ignore_case {
            left.to_ascii_lowercase()
        } else {
            *left
        };
        let right = if ignore_case {
            right.to_ascii_lowercase()
        } else {
            *right
        };
        if left != right {
            return (index, left.cmp(&right));
        }
    }
    (left.len().min(right.len()), left.len().cmp(&right.len()))
}

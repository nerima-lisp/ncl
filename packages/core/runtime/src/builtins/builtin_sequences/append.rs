use super::{exact, type_error};
use crate::value::SharedCons;
use crate::{RuntimeError, Value};
use std::collections::HashSet;

pub fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

pub fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Ok(Value::Nil);
    }
    let mut values = Vec::new();
    for argument in &arguments[..arguments.len() - 1] {
        let Some(items) = argument.list_items() else {
            return Err(type_error(function, "list", argument));
        };
        values.extend(items);
    }
    let Some(last) = arguments.last() else {
        return Ok(Value::Nil);
    };
    Ok(Value::dotted_list(values, last.clone()))
}

pub fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nconc_lists("nconc", arguments)
}

pub fn nconc_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    let Some((final_tail, preceding)) = arguments.split_last() else {
        return Ok(Value::Nil);
    };
    let mut result = None;
    let mut previous: Option<SharedCons> = None;
    for argument in preceding {
        if matches!(argument, Value::Nil | Value::Boolean(false)) {
            continue;
        }
        let Value::Cons(first) = argument else {
            return Err(type_error(function, "noncircular list", argument));
        };
        let mut last = first.clone();
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(last.identity()) {
                return Err(type_error(function, "noncircular list", argument));
            }
            let Value::Cons(next) = last.cdr() else {
                break;
            };
            last = next;
        }
        if let Some(cell) = previous {
            cell.set_cdr(argument.clone());
        } else {
            result = Some(argument.clone());
        }
        previous = Some(last);
    }
    if let Some(cell) = previous {
        cell.set_cdr(final_tail.clone());
    }
    Ok(result.unwrap_or_else(|| final_tail.clone()))
}

pub fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

pub fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreconc", 2)?;
    let cells = arguments[0]
        .list_cells()
        .ok_or_else(|| type_error("nreconc", "proper list", &arguments[0]))?;
    let mut tail = arguments[1].clone();
    for cell in cells {
        cell.set_cdr(tail);
        tail = Value::Cons(cell);
    }
    Ok(tail)
}

pub fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_returns_normalized_empty_prefix_atom() {
        let dotted = Value::dotted_list(vec![], Value::Integer(5));
        match append(&[dotted]) {
            Ok(value) => assert_eq!(value.to_string(), "5"),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn revappend_rejects_a_non_list_first_argument() {
        assert!(matches!(
            revappend(&[Value::Integer(1), Value::Nil]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn nconc_rejects_nonfinal_atoms_and_circular_lists() {
        assert!(matches!(
            nconc(&[Value::Integer(1), Value::Nil]),
            Err(RuntimeError::Type { .. })
        ));
        let cell = SharedCons::new(Value::Integer(1), Value::Nil);
        let circular = Value::Cons(cell.clone());
        cell.set_cdr(circular.clone());
        let result = nconc(&[circular, Value::Nil]);
        cell.set_cdr(Value::Nil);
        assert!(matches!(result, Err(RuntimeError::Type { .. })));
    }

    #[test]
    fn nreconc_rejects_improper_inputs_without_mutating_them() {
        let dotted = Value::cons(Value::Integer(1), Value::Integer(2));
        for input in [Value::Integer(1), dotted.clone()] {
            assert!(matches!(
                nreconc(&[input, Value::Nil]),
                Err(RuntimeError::Type { .. })
            ));
        }
        assert_eq!(dotted.to_string(), "(1 . 2)");
    }
}

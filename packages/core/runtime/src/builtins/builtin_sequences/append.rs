use super::{exact, type_error};
use crate::{RuntimeError, Value};

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
    match last {
        Value::Nil | Value::List(_) => {
            values.extend(last.list_items().unwrap_or_default());
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            if values.is_empty() && items.is_empty() {
                return Ok(last.clone());
            }
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail if values.is_empty() => Ok(tail.clone()),
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

pub fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

pub fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

pub fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
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
    fn append_returns_a_lone_dotted_list_argument_unchanged() {
        let dotted = Value::dotted_list(vec![], Value::Integer(5));
        match append(&[dotted]) {
            Ok(value) => assert_eq!(value.to_string(), "(. 5)"),
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
}

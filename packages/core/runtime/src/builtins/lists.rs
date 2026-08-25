use super::*;

pub(crate) fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

pub(crate) fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("list*", "at least one", 0));
    }
    if arguments.len() == 1 {
        return Ok(arguments[0].clone());
    }

    let mut values = arguments[..arguments.len() - 1].to_vec();
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

pub(crate) fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-list", "at least one", 0));
    }
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-list",
            "a size and keyword/value pairs",
            arguments.len(),
        ));
    }

    let size = index_argument("make-list", &arguments[0])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("make-list", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-list does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::list(vec![initial_element; size]))
}

pub(crate) fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

pub(crate) fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value => return Err(type_error("list-length", "proper list", value)),
    };
    Ok(Value::Integer(length as i64))
}

pub(crate) fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nthcdr", 2)?;
    let index = index_argument("nthcdr", &arguments[0])?;
    match &arguments[1] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(index).cloned().collect())),
        Value::DottedList { items, tail } if index < items.len() => Ok(Value::dotted_list(
            items.iter().skip(index).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { items, tail } if index == items.len() => Ok(tail.as_ref().clone()),
        value @ Value::DottedList { .. } => Err(type_error("nthcdr", "proper list", value)),
        value => Err(type_error("nthcdr", "list", value)),
    }
}

pub(crate) fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acons", 3)?;
    let Some(alist) = arguments[2].list_items() else {
        return Err(type_error("acons", "list", &arguments[2]));
    };
    let mut result = Vec::with_capacity(alist.len() + 1);
    result.push(Value::dotted_list(
        vec![arguments[0].clone()],
        arguments[1].clone(),
    ));
    result.extend(alist);
    Ok(Value::list(result))
}

pub(crate) fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("pairlis", "2 or 3", arguments.len()));
    }
    let Some(keys) = arguments[0].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[0]));
    };
    let Some(values) = arguments[1].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[1]));
    };
    if keys.len() != values.len() {
        return Err(RuntimeError::InvalidForm {
            message: "pairlis requires lists of equal length".to_string(),
            span: None,
        });
    }
    let mut result = match arguments.get(2) {
        Some(alist) => alist
            .list_items()
            .ok_or_else(|| type_error("pairlis", "list", alist))?,
        None => Vec::new(),
    };
    for (key, value) in keys.into_iter().zip(values) {
        result.insert(0, Value::dotted_list(vec![key], value));
    }
    Ok(Value::list(result))
}

pub(crate) fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cons", 2)?;
    match &arguments[1] {
        Value::Nil => Ok(Value::list(vec![arguments[0].clone()])),
        Value::List(items) => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        _ => Ok(Value::dotted_list(
            vec![arguments[0].clone()],
            arguments[1].clone(),
        )),
    }
}

pub(crate) fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "car", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        Value::DottedList { items, .. } => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        value => Err(type_error("car", "list", value)),
    }
}

pub(crate) fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cdr", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(1).cloned().collect())),
        Value::DottedList { items, tail } if items.len() > 1 => Ok(Value::dotted_list(
            items.iter().skip(1).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { tail, .. } => Ok(tail.as_ref().clone()),
        value => Err(type_error("cdr", "list", value)),
    }
}

pub(crate) fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

pub(crate) fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

pub(crate) fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

pub(crate) fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            if values.is_empty() && items.is_empty() {
                return Ok(arguments.last().expect("arguments is non-empty").clone());
            }
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail if values.is_empty() => Ok(tail.clone()),
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

pub(crate) fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

pub(crate) fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

pub(crate) fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
}

pub(crate) fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

macro_rules! list_builtins {
    () => {
fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let mut start = 0;
    let mut end = chars.len();
    let mut radix = 10_i64;
    let mut junk_allowed = false;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("parse-integer", &pair[0])?.as_str() {
            "START" => start = index_argument("parse-integer", &pair[1])?,
            "END" => end = index_argument("parse-integer", &pair[1])?,
            "RADIX" => radix = integer_argument("parse-integer", &pair[1])?,
            "JUNK-ALLOWED" => junk_allowed = pair[1].is_truthy(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("parse-integer does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > chars.len() {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer bounds are invalid".to_string(),
            span: None,
        });
    }
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
            span: None,
        });
    }
    let radix = u32::try_from(radix).expect("parse-integer radix was checked");
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
            return Ok(Value::values(vec![Value::Nil, Value::Integer(position)]));
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
        return Ok(Value::values(vec![
            Value::Integer(integer),
            Value::Integer(position),
        ]));
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    let position = i64::try_from(end).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        Value::Integer(integer),
        Value::Integer(position),
    ]))
}

fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value => return Err(type_error("list-length", "proper list", value)),
    };
    Ok(Value::Integer(length as i64))
}

fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

fn second(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("second", 1, arguments)
}

fn third(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("third", 2, arguments)
}

fn fourth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("fourth", 3, arguments)
}

fn fifth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("fifth", 4, arguments)
}

fn sixth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("sixth", 5, arguments)
}

fn seventh(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("seventh", 6, arguments)
}

fn eighth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("eighth", 7, arguments)
}

fn ninth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("ninth", 8, arguments)
}

fn tenth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("tenth", 9, arguments)
}

fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

fn nth_accessor(function: &str, index: usize, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
}

fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value if value.vector_items().is_some() => {
            value.vector_items().expect("vector has vector items").len()
        }
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    Ok(Value::Integer(length as i64))
}

fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::List(items) => items
            .get(index)
            .cloned()
            .ok_or_else(|| out_of_bounds("elt", index)),
        Value::String(value) => value
            .chars()
            .nth(index)
            .map(Value::Character)
            .ok_or_else(|| out_of_bounds("elt", index)),
        value => value
            .vector_items()
            .and_then(|items| items.get(index).cloned())
            .ok_or_else(|| {
                if value.vector_items().is_some() {
                    out_of_bounds("elt", index)
                } else {
                    type_error("elt", "sequence", value)
                }
            }),
    }
}


    };
}

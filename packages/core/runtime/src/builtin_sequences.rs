#![allow(clippy::wildcard_imports)]
use super::*;
#[allow(clippy::unnecessary_wraps)]
pub(super) fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

pub(super) fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("list*", "at least one", 0));
    }
    if arguments.len() == 1 {
        return Ok(arguments[0].clone());
    }

    let mut values = arguments[..arguments.len() - 1].to_vec();
    let Some(last) = arguments.last() else {
        return Err(arity("list*", "at least one", 0));
    };
    match last {
        Value::Nil | Value::List(_) => {
            values.extend(last.list_items().unwrap_or_default());
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

pub(super) fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    for pair in arguments[1..].as_chunks::<2>().0 {
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

pub(super) fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

pub(super) fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value => return Err(type_error("list-length", "proper list", value)),
    };
    integer_from_usize("list-length", length)
}

pub(super) fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

pub(super) fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

pub(super) fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

pub(super) fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

pub(super) fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

pub(super) fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
}

pub(super) fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

pub(super) fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) | Value::Vector(items) => items.len(),
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    integer_from_usize("length", length)
}

pub(super) fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

pub(super) fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::List(items) | Value::Vector(items) => items
            .get(index)
            .cloned()
            .ok_or_else(|| out_of_bounds("elt", index)),
        Value::String(value) => value
            .chars()
            .nth(index)
            .map(Value::Character)
            .ok_or_else(|| out_of_bounds("elt", index)),
        value => Err(type_error("elt", "sequence", value)),
    }
}

pub(super) fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

pub(super) fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

pub(super) fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

pub(super) fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

pub(super) fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

pub(super) fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

pub(super) fn compare_strings(
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

pub(super) fn string_equality(
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

pub(super) fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
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

pub(super) fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

pub(super) fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

pub(super) fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

pub(super) fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

pub(super) fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

pub(super) fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
pub(super) enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

pub(super) fn string_case_transform(
    arguments: &[Value],
    function: &str,
    case: StringCase,
) -> Result<Value, RuntimeError> {
    if !(1..=5).contains(&arguments.len()) || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(function, "1, 3, or 5", arguments.len()));
    }
    let value = string_designator(function, &arguments[0])?;
    let characters = value.chars().collect::<Vec<_>>();
    let (start, end) = sequence_bounds(function, characters.len(), &arguments[1..])?;
    let mut output = String::new();
    let mut word_start = true;
    for (index, character) in characters.into_iter().enumerate() {
        let in_range = (start..end).contains(&index);
        match case {
            StringCase::Upper if in_range => output.extend(character.to_uppercase()),
            StringCase::Lower if in_range => output.extend(character.to_lowercase()),
            StringCase::Capitalize if character.is_alphanumeric() => {
                if in_range && word_start {
                    output.extend(character.to_uppercase());
                } else if in_range {
                    output.extend(character.to_lowercase());
                } else {
                    output.push(character);
                }
                word_start = false;
            }
            StringCase::Capitalize => {
                output.push(character);
                word_start = true;
            }
            _ => output.push(character),
        }
    }
    Ok(Value::string(output))
}

pub(super) fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

pub(super) fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

pub(super) fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

pub(super) fn trim_string(
    arguments: &[Value],
    function: &str,
    trim_left: bool,
    trim_right: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let trim_set = sequence_elements(function, &arguments[0])?
        .into_iter()
        .map(|value| character_argument(function, &value))
        .collect::<Result<Vec<_>, _>>()?;
    let value = string_designator(function, &arguments[1])?;
    let characters = value.chars().collect::<Vec<_>>();
    let is_trimmed = |character: &char| trim_set.contains(character);
    let start = if trim_left {
        characters
            .iter()
            .position(|character| !is_trimmed(character))
    } else {
        Some(0)
    }
    .unwrap_or(characters.len());
    let end = if trim_right {
        characters
            .iter()
            .rposition(|character| !is_trimmed(character))
            .map_or(0, |index| index + 1)
    } else {
        characters.len()
    };
    Ok(Value::string(
        characters[start.min(end)..end].iter().collect::<String>(),
    ))
}

pub(super) fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

pub(super) fn character_designator(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        Value::String(_)
        | Value::Symbol(_)
        | Value::UninternedSymbol(_)
        | Value::Keyword(_)
        | Value::SymbolExact(_)
        | Value::KeywordExact(_) => {
            let string = string_designator(function, value)?;
            let mut characters = string.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Ok(character),
                _ => Err(type_error(function, "character designator", value)),
            }
        }
        value => Err(type_error(function, "character designator", value)),
    }
}

pub(super) fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil | Value::Boolean(false) => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::String(value)
        | Value::Symbol(value)
        | Value::UninternedSymbol(value)
        | Value::Keyword(value)
        | Value::SymbolExact(value)
        | Value::KeywordExact(value) => Ok(value.to_string()),
        Value::Character(value) => Ok(value.to_string()),
        value => Err(type_error(function, "string designator", value)),
    }
}

pub(super) fn subseq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("subseq", "2 or 3", arguments.len()));
    }
    let length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("subseq", "sequence", &arguments[0]))?;
    let start = index_argument("subseq", &arguments[1])?;
    let end = arguments
        .get(2)
        .map(|value| index_argument("subseq", value))
        .transpose()?
        .unwrap_or(length);
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: "subseq bounds are invalid".to_string(),
            span: None,
        });
    }
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items[start..end].to_vec())),
        Value::Vector(items) => Ok(Value::vector(items[start..end].to_vec())),
        Value::String(value) => {
            let result = value
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>();
            Ok(Value::string(result))
        }
        _ => Err(type_error("subseq", "sequence", &arguments[0])),
    }
}

pub(super) fn fill(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("fill", "at least two", arguments.len()));
    }
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "fill",
            "an item, a sequence, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("fill", "sequence", &arguments[1]))?;
    let (start, end) = sequence_bounds("fill", length, &arguments[2..])?;
    if matches!(arguments[1], Value::String(_)) && !matches!(arguments[0], Value::Character(_)) {
        return Err(type_error(
            "fill",
            "a character for a string",
            &arguments[0],
        ));
    }
    let mut items = sequence_elements("fill", &arguments[1])?;
    for item in &mut items[start..end] {
        *item = arguments[0].clone();
    }
    rebuild_sequence("fill", &arguments[1], items)
}

pub(super) fn replace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("replace", "at least two", arguments.len()));
    }
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "replace",
            "two sequences and keyword/value pairs",
            arguments.len(),
        ));
    }
    let first_length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[0]))?;
    let second_length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[1]))?;
    let (start1, end1, start2, end2) =
        replace_bounds(first_length, second_length, &arguments[2..])?;
    let mut result = sequence_elements("replace", &arguments[0])?;
    let source = sequence_elements("replace", &arguments[1])?;
    let count = (end1 - start1).min(end2 - start2);
    if matches!(arguments[0], Value::String(_))
        && source[start2..start2 + count]
            .iter()
            .any(|value| !matches!(value, Value::Character(_)))
    {
        return Err(type_error(
            "replace",
            "characters in the source sequence for a string destination",
            &arguments[1],
        ));
    }
    result[start1..start1 + count].clone_from_slice(&source[start2..start2 + count]);
    rebuild_sequence("replace", &arguments[0], result)
}

pub(super) fn copy_seq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-seq", 1)?;
    let items = sequence_elements("copy-seq", &arguments[0])?;
    rebuild_sequence("copy-seq", &arguments[0], items)
}

pub(super) fn concatenate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("concatenate", "at least one", 0));
    }
    let result_type = type_designator_name("concatenate", &arguments[0])?;
    let mut items = Vec::new();
    for sequence in &arguments[1..] {
        items.extend(sequence_elements("concatenate", sequence)?);
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(items)),
        "VECTOR" => Ok(Value::vector(items)),
        "STRING" | "SIMPLE-STRING" => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        "concatenate",
                        "characters for a string result",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "concatenate result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

pub(super) fn make_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 || !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "make-sequence",
            "a result type, a size, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let result_type = type_designator_name("make-sequence", &arguments[0])?;
    let size = index_argument("make-sequence", &arguments[1])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[2..].as_chunks::<2>().0 {
        match array_option_name("make-sequence", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-sequence does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(vec![initial_element; size])),
        "VECTOR" | "SIMPLE-VECTOR" => Ok(Value::vector(vec![initial_element; size])),
        "STRING" | "SIMPLE-STRING" => {
            let initial = character_argument("make-sequence", &initial_element)?;
            Ok(Value::string(
                std::iter::repeat_n(initial, size).collect::<String>(),
            ))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-sequence result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

pub(super) fn coerce(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "coerce", 2)?;
    let result_type = type_designator_name("coerce", &arguments[1])?;
    match result_type.as_str() {
        "LIST" => Ok(Value::list(sequence_elements("coerce", &arguments[0])?)),
        "VECTOR" | "SIMPLE-VECTOR" => {
            Ok(Value::vector(sequence_elements("coerce", &arguments[0])?))
        }
        "STRING" | "SIMPLE-STRING" => {
            let result = match &arguments[0] {
                Value::Nil
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
                | Value::Character(_) => string_designator("coerce", &arguments[0])?,
                value => sequence_elements("coerce", value)?
                    .into_iter()
                    .map(|item| character_argument("coerce", &item))
                    .collect::<Result<String, RuntimeError>>()?,
            };
            Ok(Value::string(result))
        }
        "SEQUENCE" => match &arguments[0] {
            Value::Nil | Value::List(_) | Value::Vector(_) | Value::String(_) => {
                Ok(arguments[0].clone())
            }
            value => Err(type_error("coerce", "a sequence", value)),
        },
        "CHARACTER" => match &arguments[0] {
            Value::Character(_) => Ok(arguments[0].clone()),
            value => Err(type_error("coerce", "a character", value)),
        },
        _ => Err(RuntimeError::InvalidForm {
            message: format!("coerce does not support result type {result_type}"),
            span: None,
        }),
    }
}

pub(super) fn sequence_bounds(
    function: &str,
    length: usize,
    options: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let mut start = 0;
    let mut end = length;
    for pair in options.as_chunks::<2>().0 {
        match array_option_name(function, &pair[0])?.as_str() {
            "START" => start = index_argument(function, &pair[1])?,
            "END" => end = index_argument(function, &pair[1])?,
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bounds are invalid"),
            span: None,
        });
    }
    Ok((start, end))
}

pub(super) fn replace_bounds(
    first_length: usize,
    second_length: usize,
    options: &[Value],
) -> Result<(usize, usize, usize, usize), RuntimeError> {
    let mut start1 = 0;
    let mut end1 = first_length;
    let mut start2 = 0;
    let mut end2 = second_length;
    for pair in options.as_chunks::<2>().0 {
        let option = array_option_name("replace", &pair[0])?;
        match option.as_str() {
            "START1" => start1 = index_argument("replace", &pair[1])?,
            "END1" => end1 = index_argument("replace", &pair[1])?,
            "START2" => start2 = index_argument("replace", &pair[1])?,
            "END2" => end2 = index_argument("replace", &pair[1])?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("replace does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start1 > end1 || end1 > first_length || start2 > end2 || end2 > second_length {
        return Err(RuntimeError::InvalidForm {
            message: "replace bounds are invalid".to_string(),
            span: None,
        });
    }
    Ok((start1, end1, start2, end2))
}

pub(super) fn sequence_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        _ => Err(type_error(function, "sequence", value)),
    }
}

pub(super) fn rebuild_sequence(
    function: &str,
    template: &Value,
    items: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match template {
        Value::Nil | Value::List(_) => Ok(Value::list(items)),
        Value::Vector(_) => Ok(Value::vector(items)),
        Value::String(_) => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        function,
                        "characters for a string sequence",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        value => Err(type_error(function, "sequence", value)),
    }
}

pub(super) fn getf(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("getf", "2 or 3", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("getf", "property list", &arguments[0]));
    };
    if !items.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: None,
        });
    }
    for pair in items.as_chunks::<2>().0 {
        if arguments[1].eq_value(&pair[0]) {
            return Ok(pair[1].clone());
        }
    }
    Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
}

pub(super) fn get_properties(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-properties", 2)?;
    let Some(plist) = arguments[0].list_items() else {
        return Err(type_error("get-properties", "property list", &arguments[0]));
    };
    let Some(indicators) = arguments[1].list_items() else {
        return Err(type_error("get-properties", "list", &arguments[1]));
    };
    if !plist.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "get-properties requires an even-length property list".to_string(),
            span: None,
        });
    }
    for (index, pair) in plist.as_chunks::<2>().0.iter().enumerate() {
        if indicators
            .iter()
            .any(|indicator| indicator.eq_value(&pair[0]))
        {
            return Ok(Value::values(vec![
                pair[0].clone(),
                pair[1].clone(),
                Value::list(plist[index * 2..].to_vec()),
            ]));
        }
    }
    Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil]))
}

pub(super) fn sequence_length(value: &Value) -> Option<usize> {
    match value {
        Value::Nil => Some(0),
        Value::List(items) | Value::Vector(items) => Some(items.len()),
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

pub(super) fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

pub(super) fn integer_from_usize(function: &str, value: usize) -> Result<Value, RuntimeError> {
    i64::try_from(value)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::InvalidForm {
            message: format!("{function} result is too large for an integer"),
            span: None,
        })
}

pub(super) fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

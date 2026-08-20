macro_rules! string_builtins {
    () => {
fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

fn compare_strings(
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
        Ok(Value::Integer(index as i64))
    } else {
        Ok(Value::Nil)
    }
}

fn string_equality(
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

fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
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

fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

fn string_case_transform(
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

fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

fn trim_string(
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

fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

fn character_designator(function: &str, value: &Value) -> Result<char, RuntimeError> {
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

fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::Boolean(false) => Ok("NIL".to_string()),
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

fn subseq(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    let items = sequence_elements("subseq", &arguments[0])?;
    rebuild_sequence("subseq", &arguments[0], items[start..end].to_vec())
}

fn fill(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn replace(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    result[start1..(count + start1)].clone_from_slice(&source[start2..(count + start2)]);
    rebuild_sequence("replace", &arguments[0], result)
}

fn copy_seq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-seq", 1)?;
    let items = sequence_elements("copy-seq", &arguments[0])?;
    rebuild_sequence("copy-seq", &arguments[0], items)
}

fn concatenate(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
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

fn make_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    for pair in arguments[2..].chunks_exact(2) {
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
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
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

fn coerce(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "coerce", 2)?;
    let result_type = type_designator_name("coerce", &arguments[1])?;
    match result_type.as_str() {
        "LIST" => Ok(Value::list(sequence_elements("coerce", &arguments[0])?)),
        "VECTOR" | "SIMPLE-VECTOR" => {
            Ok(Value::vector(sequence_elements("coerce", &arguments[0])?))
        }
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
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
        "SEQUENCE" => {
            if sequence_length(&arguments[0]).is_some() {
                Ok(arguments[0].clone())
            } else {
                Err(type_error("coerce", "a sequence", &arguments[0]))
            }
        }
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

fn sequence_bounds(
    function: &str,
    length: usize,
    options: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let mut start = 0;
    let mut end = length;
    for pair in options.chunks_exact(2) {
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

fn replace_bounds(
    first_length: usize,
    second_length: usize,
    options: &[Value],
) -> Result<(usize, usize, usize, usize), RuntimeError> {
    let mut start1 = 0;
    let mut end1 = first_length;
    let mut start2 = 0;
    let mut end2 = second_length;
    for pair in options.chunks_exact(2) {
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

fn sequence_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    if let Some(items) = sequence_items(value) {
        return Ok(items);
    }
    match value {
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        _ => Err(type_error(function, "sequence", value)),
    }
}

fn rebuild_sequence(
    function: &str,
    template: &Value,
    items: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match template {
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
        value if value.list_items().is_some() => Ok(Value::list(items)),
        value if value.vector_items().is_some() => Ok(Value::vector(items)),
        value => Err(type_error(function, "sequence", value)),
    }
}

fn getf(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("getf", "2 or 3", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("getf", "property list", &arguments[0]));
    };
    if items.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: None,
        });
    }
    for pair in items.chunks_exact(2) {
        if arguments[1].eq_value(&pair[0]) {
            return Ok(pair[1].clone());
        }
    }
    Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
}

fn get_properties(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-properties", 2)?;
    let Some(plist) = arguments[0].list_items() else {
        return Err(type_error("get-properties", "property list", &arguments[0]));
    };
    let Some(indicators) = arguments[1].list_items() else {
        return Err(type_error("get-properties", "list", &arguments[1]));
    };
    if plist.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "get-properties requires an even-length property list".to_string(),
            span: None,
        });
    }
    for (index, pair) in plist.chunks_exact(2).enumerate() {
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

fn sequence_length(value: &Value) -> Option<usize> {
    if let Some(items) = sequence_items(value) {
        return Some(items.len());
    }
    match value {
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        value if value.list_items().is_some() => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-name", 1)?;
    let name = match &arguments[0] {
        Value::UninternedSymbol(name) => name.to_string(),
        value => {
            let name = value
                .symbol_name()
                .ok_or_else(|| type_error("symbol-name", "a symbol", &arguments[0]))?;
            let name = match package::split_symbol(name) {
                Some((_, symbol_name, _)) => symbol_name,
                None => name,
            };
            name.to_string()
        }
    };
    Ok(Value::string(name))
}

fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-package", 1)?;
    let package_name = match &arguments[0] {
        Value::UninternedSymbol(_) => return Ok(Value::Nil),
        Value::Keyword(_) | Value::KeywordExact(_) => KEYWORD_PACKAGE.to_string(),
        Value::Nil | Value::Boolean(_) => COMMON_LISP_PACKAGE.to_string(),
        Value::Symbol(name) | Value::SymbolExact(name) => {
            match package::split_symbol(name.as_ref()) {
                Some((package_name, _, _)) => package::normalize_package_name(package_name),
                None => package::DEFAULT_PACKAGE.to_string(),
            }
        }
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::symbol(package_name))
}

fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(arguments[0].vector_items().is_some()))
}

fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(arguments[0].vector_items().is_some()))
}

fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fill-pointer", 1)?;
    arguments[0]
        .vector_fill_pointer()
        .map(|fill_pointer| Value::Integer(fill_pointer as i64))
        .ok_or_else(|| type_error("fill-pointer", "vector with fill pointer", &arguments[0]))
}

fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

fn simple_condition_format_control(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-control", 1)?;
    arguments[0]
        .simple_condition_format_control()
        .map(|control| Value::string(control.to_owned()))
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-control",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

fn simple_condition_format_arguments(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-arguments", 1)?;
    arguments[0]
        .simple_condition_format_arguments()
        .map(Value::list)
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-arguments",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}


    };
}

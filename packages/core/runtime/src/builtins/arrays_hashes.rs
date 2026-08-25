use super::*;

pub(crate) fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    reverse_list("reverse", &arguments[0])
}

pub(crate) fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    reverse_list("nreverse", &arguments[0])
}

pub(crate) fn reverse_list(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let Some(mut items) = value.list_items() else {
        return Err(type_error(function, "list", value));
    };
    items.reverse();
    Ok(Value::list(items))
}

pub(crate) fn last(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("last", "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("last", "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument("last", value))
        .transpose()?
        .unwrap_or(1);
    if count == 0 {
        return Ok(Value::Nil);
    }
    let start = items.len().saturating_sub(count);
    Ok(Value::list(items[start..].to_vec()))
}

pub(crate) fn butlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("butlast", arguments)
}

pub(crate) fn nbutlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("nbutlast", arguments)
}

pub(crate) fn butlast_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument(function, value))
        .transpose()?
        .unwrap_or(1);
    let end = items.len().saturating_sub(count);
    Ok(Value::list(items[..end].to_vec()))
}

pub(crate) fn copy_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-list", 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("copy-list", "list", &arguments[0]));
    };
    Ok(Value::list(items))
}

pub(crate) fn copy_alist(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-alist", 1)?;
    let Some(entries) = arguments[0].list_items() else {
        return Err(type_error("copy-alist", "association list", &arguments[0]));
    };
    let copied = entries
        .into_iter()
        .map(|entry| match entry {
            Value::List(items) => Ok(Value::list(items.as_ref().clone())),
            Value::DottedList { items, tail } => Ok(Value::dotted_list(
                items.as_ref().clone(),
                tail.as_ref().clone(),
            )),
            value => Err(type_error("copy-alist", "association", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::list(copied))
}

pub(crate) fn copy_tree(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-tree", 1)?;
    Ok(copy_tree_value(&arguments[0]))
}

pub(crate) fn copy_tree_value(value: &Value) -> Value {
    match value {
        Value::List(items) => Value::list(items.iter().map(copy_tree_value).collect()),
        Value::DottedList { items, tail } => Value::dotted_list(
            items.iter().map(copy_tree_value).collect(),
            copy_tree_value(tail),
        ),
        _ => value.clone(),
    }
}

pub(crate) fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

pub(crate) fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].chunks_exact(2) {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        vec![initial_element.unwrap_or(Value::Nil); total_size]
    };
    if dimensions.len() == 1 {
        Ok(Value::vector(elements))
    } else {
        Ok(Value::array(dimensions, elements))
    }
}

pub(crate) fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("aref", "at least one", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("aref", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "aref",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("aref", &dimensions, &arguments[1..])?;
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("aref", index))
}

pub(crate) fn svref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "svref", 2)?;
    let index = index_argument("svref", &arguments[1])?;
    let Value::Vector(items) = &arguments[0] else {
        return Err(type_error("svref", "simple-vector", &arguments[0]));
    };
    items
        .get(index)
        .cloned()
        .ok_or_else(|| out_of_bounds("svref", index))
}

pub(crate) fn bit_value(function: &str, value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Integer(bit) if *bit == 0 || *bit == 1 => Ok(()),
        _ => Err(type_error(function, "bit", value)),
    }
}

pub(crate) fn bit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("bit", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("bit", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "bit",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("bit", &dimensions, &arguments[1..])?;
    let value = array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("bit", index))?;
    bit_value("bit", &value)?;
    Ok(value)
}

pub(crate) fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "row-major-aref", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("row-major-aref", "array", &arguments[0]))?;
    let index = index_argument("row-major-aref", &arguments[1])?;
    let total_size = array_total_size_for("row-major-aref", &dimensions)?;
    if index >= total_size {
        return Err(out_of_bounds("row-major-aref", index));
    }
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("row-major-aref", index))
}

pub(crate) fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-row-major-index", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-row-major-index", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-row-major-index",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    Ok(Value::Integer(
        array_coordinate_index("array-row-major-index", &dimensions, &arguments[1..])? as i64,
    ))
}

pub(crate) fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-in-bounds-p", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-in-bounds-p", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-in-bounds-p",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    for (dimension, value) in dimensions.iter().zip(&arguments[1..]) {
        if index_argument("array-in-bounds-p", value)? >= *dimension {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

pub(crate) fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(Value::symbol("T"))
}

pub(crate) fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

pub(crate) fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

pub(crate) fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    Ok(Value::Integer(dimensions.len() as i64))
}

pub(crate) fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimensions", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimensions", "array", &arguments[0]))?;
    Ok(Value::list(
        dimensions
            .into_iter()
            .map(|dimension| Value::Integer(dimension as i64))
            .collect(),
    ))
}

pub(crate) fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimension", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimension", "array", &arguments[0]))?;
    let index = index_argument("array-dimension", &arguments[1])?;
    dimensions
        .get(index)
        .copied()
        .map(|dimension| Value::Integer(dimension as i64))
        .ok_or_else(|| out_of_bounds("array-dimension", index))
}

pub(crate) fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    Ok(Value::Integer(
        array_total_size_for("array-total-size", &dimensions)? as i64,
    ))
}

pub(crate) fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(arity(
            "make-hash-table",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut test = "EQL".to_string();
    for pair in arguments.chunks_exact(2) {
        let name = hash_table_option_name("make-hash-table", &pair[0])?;
        match name.as_str() {
            "TEST" => test = hash_table_test_name("make-hash-table", &pair[1])?,
            "SIZE" => {
                index_argument("make-hash-table", &pair[1])?;
            }
            "REHASH-SIZE" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                if value.as_float() <= 0.0 {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-size must be positive".to_string(),
                        span: None,
                    });
                }
            }
            "REHASH-THRESHOLD" => {
                let value = number_argument("make-hash-table", &pair[1])?.as_float();
                if !(0.0..=1.0).contains(&value) {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-threshold must be between 0 and 1"
                            .to_string(),
                        span: None,
                    });
                }
            }
            "SYNCHRONIZED" => {
                if !matches!(pair[1], Value::Nil | Value::Boolean(_)) {
                    return Err(type_error(
                        "make-hash-table",
                        "boolean for :synchronized",
                        &pair[1],
                    ));
                }
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-hash-table does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::hash_table(test))
}

pub(crate) fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 && arguments.len() != 3 {
        return Err(arity("gethash", "two or three", arguments.len()));
    }
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let key = &arguments[0];
    let found = entries
        .borrow()
        .iter()
        .find(|(stored_key, _)| hash_table_key_equal(&test, stored_key, key))
        .map(|(_, value)| value.clone());
    match found {
        Some(value) => Ok(Value::values(vec![value, Value::boolean(true)])),
        None => Ok(Value::values(vec![
            arguments.get(2).cloned().unwrap_or(Value::Nil),
            Value::Nil,
        ])),
    }
}

pub(crate) fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "remhash", 2)?;
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let key = &arguments[0];
    let mut entries = entries.borrow_mut();
    let previous_length = entries.len();
    entries.retain(|(stored_key, _)| !hash_table_key_equal(&test, stored_key, key));
    Ok(Value::boolean(entries.len() != previous_length))
}

pub(crate) fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

pub(crate) fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

pub(crate) fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    Ok(Value::Integer(entries.borrow().len() as i64))
}

pub(crate) fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

pub(crate) fn hash_table_option_name(
    function: &str,
    value: &Value,
) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

pub(crate) fn hash_table_test_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let name = match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => normalize_name(name),
        Value::Function(function_value) => match function_value.as_ref() {
            Function::Builtin { name, .. } | Function::Primitive { name } => normalize_name(name),
            _ => {
                return Err(type_error(
                    function,
                    "named hash-table test function",
                    value,
                ));
            }
        },
        other => return Err(type_error(function, "hash-table test designator", other)),
    };
    if matches!(name.as_str(), "EQ" | "EQL" | "EQUAL" | "EQUALP") {
        Ok(name)
    } else {
        Err(RuntimeError::InvalidForm {
            message: format!("{function} :test must be EQ, EQL, EQUAL, or EQUALP, got {name}"),
            span: None,
        })
    }
}

pub(crate) fn hash_table_key_equal(test: &str, left: &Value, right: &Value) -> bool {
    match test {
        "EQ" => left.eq_value(right),
        "EQUAL" => left.equal_value(right),
        "EQUALP" => equalp_value(left, right),
        _ => eql_value(left, right),
    }
}

pub(crate) fn parse_array_dimensions(
    function: &str,
    value: &Value,
) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        Value::List(_) | Value::Vector(_) => {
            let items = sequence_items(value).expect("list or vector has sequence items");
            items
                .iter()
                .map(|item| index_argument(function, item))
                .collect()
        }
        other => Err(type_error(
            function,
            "integer or sequence of integers",
            other,
        )),
    }
}

pub(crate) fn array_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

pub(crate) fn flatten_array_contents(
    function: &str,
    contents: &Value,
    dimensions: &[usize],
    output: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    if dimensions.is_empty() {
        output.push(contents.clone());
        return Ok(());
    }
    let Some(items) = sequence_items(contents) else {
        return Err(type_error(
            function,
            "nested sequence for :initial-contents",
            contents,
        ));
    };
    if items.len() != dimensions[0] {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} :initial-contents expected {} elements, got {}",
                dimensions[0],
                items.len()
            ),
            span: None,
        });
    }
    if dimensions.len() == 1 {
        output.extend(items);
    } else {
        for item in items {
            flatten_array_contents(function, &item, &dimensions[1..], output)?;
        }
    }
    Ok(())
}

pub(crate) fn array_coordinate_index(
    function: &str,
    dimensions: &[usize],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    let mut offset: usize = 0;
    for (axis, (dimension, value)) in dimensions.iter().zip(indices).enumerate() {
        let index = index_argument(function, value)?;
        if index >= *dimension {
            return Err(out_of_bounds(function, index));
        }
        let stride = dimensions[axis + 1..]
            .iter()
            .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        let contribution = index
            .checked_mul(stride)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        offset = offset
            .checked_add(contribution)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
    }
    Ok(offset)
}

pub(crate) fn array_total_size_for(
    function: &str,
    dimensions: &[usize],
) -> Result<usize, RuntimeError> {
    dimensions.iter().try_fold(1_usize, |total, dimension| {
        total
            .checked_mul(*dimension)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} array is too large"),
                span: None,
            })
    })
}

pub(crate) fn dimensions_for_array(value: &Value) -> Option<Vec<usize>> {
    match value {
        Value::Vector(items) => Some(vec![items.len()]),
        Value::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
        _ => None,
    }
}

pub(crate) fn array_elements(value: &Value) -> Option<Vec<Value>> {
    value.vector_items().or_else(|| value.array_items())
}

pub(crate) fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value.list_items().or_else(|| value.vector_items())
}

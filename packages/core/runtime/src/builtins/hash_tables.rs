use super::*;

pub(super) fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() % 2 != 0 {
        return Err(arity(
            "make-hash-table",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut test = "EQL".to_string();
    let mut size = 16;
    let mut rehash_size = 1.5;
    let mut rehash_threshold = 0.75;
    let mut synchronized = false;
    for pair in arguments.chunks_exact(2) {
        let name = hash_table_option_name("make-hash-table", &pair[0])?;
        match name.as_str() {
            "TEST" => test = hash_table_test_name("make-hash-table", &pair[1])?,
            "SIZE" => {
                size = index_argument("make-hash-table", &pair[1])?;
            }
            "REHASH-SIZE" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                if value.as_float() <= 0.0 {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-size must be positive".to_string(),
                        span: None,
                    });
                }
                rehash_size = value.as_float();
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
                rehash_threshold = value;
            }
            "SYNCHRONIZED" => {
                if !matches!(pair[1], Value::Nil | Value::Boolean(_)) {
                    return Err(type_error(
                        "make-hash-table",
                        "boolean for :synchronized",
                        &pair[1],
                    ));
                }
                synchronized = pair[1].is_truthy();
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-hash-table does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::hash_table_with_options(
        test,
        size,
        rehash_size,
        rehash_threshold,
        synchronized,
    ))
}

pub(super) fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

pub(super) fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

pub(super) fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    Ok(Value::Integer(entries.borrow().len() as i64))
}

pub(super) fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

pub(super) fn hash_table_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-size", 1)?;
    let table = &arguments[0];
    let Some(size) = table.hash_table_size() else {
        return Err(type_error("hash-table-size", "hash-table", table));
    };
    Ok(Value::Integer(size as i64))
}

pub(super) fn hash_table_rehash_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-rehash-size", 1)?;
    let table = &arguments[0];
    let Some(size) = table.hash_table_rehash_size() else {
        return Err(type_error("hash-table-rehash-size", "hash-table", table));
    };
    Ok(Value::Float(size))
}

pub(super) fn hash_table_rehash_threshold(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-rehash-threshold", 1)?;
    let table = &arguments[0];
    let Some(threshold) = table.hash_table_rehash_threshold() else {
        return Err(type_error(
            "hash-table-rehash-threshold",
            "hash-table",
            table,
        ));
    };
    Ok(Value::Float(threshold))
}

pub(super) fn hash_table_synchronized_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-synchronized-p", 1)?;
    let table = &arguments[0];
    let Some(synchronized) = table.hash_table_synchronized() else {
        return Err(type_error("hash-table-synchronized-p", "hash-table", table));
    };
    Ok(Value::boolean(synchronized))
}

pub(super) fn make_hash_table_iterator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL-MAKE-HASH-TABLE-ITERATOR", 1)?;
    Value::hash_table_iterator(&arguments[0]).ok_or_else(|| {
        type_error(
            "__NCL-MAKE-HASH-TABLE-ITERATOR",
            "hash-table",
            &arguments[0],
        )
    })
}

pub(super) fn hash_table_iterator_next(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL-HASH-TABLE-ITERATOR-NEXT", 1)?;
    arguments[0].hash_table_iterator_next().ok_or_else(|| {
        type_error(
            "__NCL-HASH-TABLE-ITERATOR-NEXT",
            "hash-table-iterator",
            &arguments[0],
        )
    })
}

fn hash_table_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        Value::QualifiedSymbolExact {
            reference,
            package_len,
        } => Ok(normalize_name(&reference[*package_len + 2..])),
        other => Err(type_error(function, "keyword", other)),
    }
}

fn hash_table_test_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let name = match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => normalize_name(name),
        Value::QualifiedSymbolExact {
            reference,
            package_len,
        } => normalize_name(&reference[*package_len + 2..]),
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

fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    Ok(Value::Integer(entries.borrow().len() as i64))
}

fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

fn hash_table_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
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

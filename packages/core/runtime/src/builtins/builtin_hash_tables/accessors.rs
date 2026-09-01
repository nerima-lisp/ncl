use super::designators::hash_table_key_equal;
use crate::builtins::builtin_helpers::{arity, exact, type_error};
use crate::builtins::integer_from_usize;
use crate::{RuntimeError, Value};

pub fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 && arguments.len() != 3 {
        return Err(arity("gethash", "two or three", arguments.len()));
    }
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let test = test.to_string();
    let key = &arguments[0];
    let found = entries
        .borrow()
        .iter()
        .find(|(stored_key, _)| hash_table_key_equal(&test, stored_key, key))
        .map(|(_, value)| value.clone());
    found.map_or_else(
        || {
            Ok(Value::values(vec![
                arguments.get(2).cloned().unwrap_or(Value::Nil),
                Value::Nil,
            ]))
        },
        |value| Ok(Value::values(vec![value, Value::boolean(true)])),
    )
}

pub fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "remhash", 2)?;
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let test = test.to_string();
    let key = &arguments[0];
    let mut entries = entries.borrow_mut();
    let previous_length = entries.len();
    entries.retain(|(stored_key, _)| !hash_table_key_equal(&test, stored_key, key));
    Ok(Value::boolean(entries.len() != previous_length))
}

pub fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

pub fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

pub fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    integer_from_usize("hash-table-count", entries.borrow().len())
}

pub fn hash_table_keys(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ncl-hash-table-keys", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("ncl-hash-table-keys", "hash-table", table));
    };
    Ok(Value::list(
        entries
            .borrow()
            .iter()
            .map(|(key, _)| key.clone())
            .collect(),
    ))
}

pub fn hash_table_values(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ncl-hash-table-values", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("ncl-hash-table-values", "hash-table", table));
    };
    Ok(Value::list(
        entries
            .borrow()
            .iter()
            .map(|(_, value)| value.clone())
            .collect(),
    ))
}

pub fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

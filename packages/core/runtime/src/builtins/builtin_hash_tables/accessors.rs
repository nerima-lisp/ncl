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

pub(crate) fn set_hash_table_entry(table: &Value, key: Value, value: Value) -> bool {
    let Some(test) = table.hash_table_test() else {
        return false;
    };
    let Some(entries) = table.hash_table_entries() else {
        return false;
    };
    let test = test.to_string();
    let mut entries = entries.borrow_mut();
    if let Some((_, slot)) = entries
        .iter_mut()
        .find(|(stored_key, _)| hash_table_key_equal(&test, stored_key, &key))
    {
        *slot = value;
        return true;
    }
    entries.push((key, value));
    let entry_count = entries.len();
    drop(entries);
    rehash_hash_table(table, entry_count);
    true
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Hash-table rehash thresholds are specified as real numbers"
)]
fn rehash_hash_table(table: &Value, entry_count: usize) {
    let Some(current_size) = table.hash_table_size() else {
        return;
    };
    let Some(threshold_value) = table.hash_table_rehash_threshold() else {
        return;
    };
    let threshold = numeric_value(threshold_value).unwrap_or(1.0);
    if !threshold.is_finite()
        || (threshold > 0.0 && (entry_count as f64) <= current_size as f64 * threshold)
    {
        return;
    }
    let Some(growth_value) = table.hash_table_rehash_size() else {
        return;
    };
    let growth = numeric_value(growth_value).unwrap_or(1.0);
    let mut next_size = next_hash_table_size(current_size, growth_value, growth);
    while threshold > 0.0
        && (entry_count as f64) > next_size as f64 * threshold
        && next_size < usize::MAX
    {
        let grown = next_hash_table_size(next_size, growth_value, growth);
        if grown == next_size {
            break;
        }
        next_size = grown;
    }
    let _ = table.set_hash_table_size(next_size);
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Hash-table capacities are scaled by a real rehash-size value"
)]
fn next_hash_table_size(current_size: usize, growth_value: &Value, growth: f64) -> usize {
    if current_size == usize::MAX {
        return current_size;
    }
    let next_size = if matches!(growth_value, Value::Float(_)) {
        rounded_capacity(current_size as f64 * growth)
    } else {
        current_size.saturating_add(rounded_capacity(growth))
    };
    next_size.max(current_size.saturating_add(1))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Hash-table capacities are represented as usize values"
)]
fn rounded_capacity(value: f64) -> usize {
    if !value.is_finite() || value >= usize::MAX as f64 {
        usize::MAX
    } else {
        value.ceil().max(1.0) as usize
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Hash-table rehash options are converted to floating-point values"
)]
fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(value) => Some(*value as f64),
        Value::BigInteger(value) => value.to_string().parse().ok(),
        Value::Rational(value) => Some(value.numerator_f64() / value.denominator_f64()),
        Value::BigRational(value) => {
            let numerator = value.numerator().to_string().parse::<f64>().ok()?;
            let denominator = value.denominator().to_string().parse::<f64>().ok()?;
            Some(numerator / denominator)
        }
        Value::Float(value) => Some(*value),
        _ => None,
    }
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

pub fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

pub fn hash_table_size_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-size", 1)?;
    let table = &arguments[0];
    let Some(size) = table.hash_table_size() else {
        return Err(type_error("hash-table-size", "hash-table", table));
    };
    integer_from_usize("hash-table-size", size)
}

pub fn hash_table_rehash_size_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-rehash-size", 1)?;
    let table = &arguments[0];
    let Some(value) = table.hash_table_rehash_size() else {
        return Err(type_error("hash-table-rehash-size", "hash-table", table));
    };
    Ok(value.clone())
}

pub fn hash_table_rehash_threshold_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-rehash-threshold", 1)?;
    let table = &arguments[0];
    let Some(value) = table.hash_table_rehash_threshold() else {
        return Err(type_error(
            "hash-table-rehash-threshold",
            "hash-table",
            table,
        ));
    };
    Ok(value.clone())
}

pub fn hash_table_weakness_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-weakness", 1)?;
    let table = &arguments[0];
    let Some(weakness) = table.hash_table_weakness() else {
        return Err(type_error("hash-table-weakness", "hash-table", table));
    };
    Ok(weakness.map_or(Value::Nil, Value::keyword))
}

pub fn hash_table_keys(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-keys", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-keys", "hash-table", table));
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
    exact(arguments, "hash-table-values", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-values", "hash-table", table));
    };
    Ok(Value::list(
        entries
            .borrow()
            .iter()
            .map(|(_, value)| value.clone())
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::set_hash_table_entry;
    use crate::Value;

    #[test]
    fn grows_hash_table_capacity_when_threshold_is_crossed() {
        let table =
            Value::hash_table_with_options("EQL", 2, Value::Integer(2), Value::Float(0.5), None);

        assert!(set_hash_table_entry(
            &table,
            Value::Integer(1),
            Value::Integer(10)
        ));
        assert_eq!(table.hash_table_size(), Some(2));
        assert!(set_hash_table_entry(
            &table,
            Value::Integer(2),
            Value::Integer(20)
        ));
        assert_eq!(table.hash_table_size(), Some(4));
        assert!(set_hash_table_entry(
            &table,
            Value::Integer(2),
            Value::Integer(21)
        ));
        assert_eq!(table.hash_table_size(), Some(4));
    }
}

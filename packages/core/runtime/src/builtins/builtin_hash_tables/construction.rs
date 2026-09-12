use super::designators::{hash_table_option_name, hash_table_test_name};
use crate::builtins::builtin_helpers::{arity, type_error};
use crate::builtins::{index_argument, number_argument};
use crate::{RuntimeError, Value};
use std::rc::Rc;

pub fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(arity(
            "make-hash-table",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut test = "EQL".to_string();
    let mut size = 16;
    let mut rehash_size = Value::Float(1.5);
    let mut rehash_threshold = Value::Float(1.0);
    let mut weakness = None;
    for pair in arguments.as_chunks::<2>().0 {
        let name = hash_table_option_name("make-hash-table", &pair[0])?;
        match name.as_str() {
            "TEST" => test = hash_table_test_name("make-hash-table", &pair[1])?,
            "SIZE" => {
                size = index_argument("make-hash-table", &pair[1])?.max(1);
            }
            "REHASH-SIZE" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                let numeric = value.as_float();
                if !numeric.is_finite() || numeric <= 0.0 {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-size must be positive".to_string(),
                        span: None,
                    });
                }
                rehash_size = pair[1].clone();
            }
            "REHASH-THRESHOLD" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                let numeric = value.as_float();
                if !numeric.is_finite() || !(0.0..=1.0).contains(&numeric) {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-threshold must be between 0 and 1"
                            .to_string(),
                        span: None,
                    });
                }
                rehash_threshold = pair[1].clone();
            }
            "WEAKNESS" => {
                if matches!(pair[1], Value::Nil | Value::Boolean(false)) {
                    weakness = None;
                } else {
                    let value = hash_table_option_name("make-hash-table", &pair[1])?;
                    if !matches!(value.as_str(), "KEY" | "VALUE" | "KEY-AND-VALUE") {
                        return Err(RuntimeError::InvalidForm {
                            message: format!(
                                "make-hash-table :weakness must be NIL, :KEY, :VALUE, or :KEY-AND-VALUE, got :{value}"
                            ),
                            span: None,
                        });
                    }
                    weakness = Some(Rc::from(value));
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
    Ok(Value::hash_table_with_options(
        test,
        size,
        rehash_size,
        rehash_threshold,
        weakness,
    ))
}

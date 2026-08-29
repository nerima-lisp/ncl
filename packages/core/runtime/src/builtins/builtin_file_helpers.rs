use std::path::PathBuf;

use super::type_error;
use crate::environment::normalize_name;
use crate::{RuntimeError, Value};

pub(super) fn pathname_argument(function: &str, value: &Value) -> Result<PathBuf, RuntimeError> {
    match value {
        Value::String(value) => Ok(PathBuf::from(value.as_ref())),
        value => Err(type_error(function, "a string pathname", value)),
    }
}

pub(super) fn stream_keyword_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Ok(normalize_name(name)),
        value => Err(type_error(function, "a keyword", value)),
    }
}

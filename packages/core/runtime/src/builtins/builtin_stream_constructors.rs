use super::{arity, exact, integer_argument, type_error};
use crate::{RuntimeError, Value};

pub(super) fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=3).contains(&arguments.len()) {
        return Err(arity("make-string-input-stream", "1 to 3", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("make-string-input-stream", "a string", value)),
    };
    let length = source.chars().count();
    let start = match arguments.get(1) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => 0,
    };
    let end = match arguments.get(2) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => length,
    };
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "make-string-input-stream start must not exceed end".to_string(),
            span: None,
        });
    }
    Ok(Value::string_input_stream(source, start, end))
}

pub(super) fn stream_bound(
    function: &str,
    value: &Value,
    length: usize,
) -> Result<usize, RuntimeError> {
    let bound = integer_argument(function, value)?;
    let bound = usize::try_from(bound).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} stream position must be non-negative"),
        span: None,
    })?;
    if bound > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} stream position is outside the string"),
            span: None,
        });
    }
    Ok(bound)
}

pub(super) fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-string-output-stream", 0)?;
    Ok(Value::string_output_stream())
}

use super::{arity, exact, integer_argument, stream_keyword_name, type_error};
use crate::{RuntimeError, Value};

pub(crate) fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-string-input-stream", "a string and optional bounds", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("make-string-input-stream", "a string", value)),
    };
    let length = source.chars().count();
    let (mut start, mut end) = (0, length);
    let options = &arguments[1..];
    if options.len() == 1 || (options.len() > 2 && !options.len().is_multiple_of(2)) {
        return Err(arity("make-string-input-stream", "a string followed by bounds or keyword/value pairs", arguments.len()));
    }
    if options.len() == 2 && !matches!(options[0], Value::Keyword(_) | Value::KeywordExact(_)) {
        start = stream_bound("make-string-input-stream", &options[0], length)?;
        end = stream_bound("make-string-input-stream", &options[1], length)?;
    } else if !options.is_empty() {
        for pair in options.chunks_exact(2) {
            let name = stream_keyword_name("make-string-input-stream", &pair[0])?;
            match name.as_str() {
                "START" => start = stream_bound("make-string-input-stream", &pair[1], length)?,
                "END" => end = stream_bound("make-string-input-stream", &pair[1], length)?,
                _ => return Err(RuntimeError::InvalidForm { message: format!("make-string-input-stream does not support keyword :{name}"), span: None }),
            }
        }
    }
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

pub(crate) fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-string-output-stream", 0)?;
    Ok(Value::string_output_stream())
}

use super::{
    arity, exact, integer_from_usize, integer_value, stream_keyword_name, stream_reference,
    stream_state_error, type_error,
};
use crate::{RuntimeError, Stream, Value};

enum PositionRequest {
    Query,
    Set(Option<usize>),
}

fn position_request(
    function: &str,
    stream: &Stream,
    value: &Value,
) -> Result<PositionRequest, RuntimeError> {
    if matches!(value, Value::Nil) {
        return Ok(PositionRequest::Query);
    }
    let is_keyword = match value {
        Value::Keyword(_) | Value::KeywordExact(_) => true,
        Value::InternedSymbol(symbol) => symbol.keyword(),
        _ => false,
    };
    if is_keyword {
        let name = stream_keyword_name(function, value)?;
        return match name.as_str() {
            "START" => Ok(PositionRequest::Set(Some(0))),
            "END" => Ok(PositionRequest::Set(stream.file_end_position())),
            _ => Err(RuntimeError::InvalidForm {
                message: format!("{function} does not support position :{name}"),
                span: None,
            }),
        };
    }
    let position = integer_value(function, value)?;
    if position < ibig::IBig::from(0) {
        return Err(type_error(function, "a non-negative integer", value));
    }
    let position = usize::try_from(position)
        .map_err(|_| type_error(function, "a representable non-negative integer", value))?;
    Ok(PositionRequest::Set(Some(position)))
}

pub(crate) fn file_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("file-position", "1 to 2", arguments.len()));
    }
    let stream = stream_reference("file-position", &arguments[0])?;
    if !stream.borrow().is_open() {
        return Err(stream_state_error("file-position", "an open stream"));
    }
    let request = match arguments.get(1) {
        None => PositionRequest::Query,
        Some(value) => {
            let stream = stream.borrow();
            position_request("file-position", &stream, value)?
        }
    };
    let mut stream = stream.borrow_mut();
    match request {
        PositionRequest::Query => stream
            .file_position()
            .map(|position| integer_from_usize("file-position", position))
            .transpose()
            .map(|position| position.unwrap_or(Value::Nil)),
        PositionRequest::Set(Some(position)) => {
            Ok(Value::boolean(stream.set_file_position(position)))
        }
        PositionRequest::Set(None) => Ok(Value::Nil),
    }
}

pub(crate) fn file_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-length", 1)?;
    let stream = stream_reference("file-length", &arguments[0])?;
    let stream = stream.borrow();
    if !stream.is_file_stream() {
        return Err(type_error("file-length", "a file stream", &arguments[0]));
    }
    if !stream.is_open() {
        return Err(stream_state_error("file-length", "an open stream"));
    }
    stream
        .file_end_position()
        .map(|position| integer_from_usize("file-length", position))
        .transpose()
        .map(|position| position.unwrap_or(Value::Nil))
}

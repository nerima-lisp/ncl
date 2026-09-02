use super::{arity, exact, index_argument, stream_keyword_name, stream_reference, stream_state_error};
use crate::{RuntimeError, Value};

pub(crate) fn close_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 3 {
        return Err(arity("close", "1 or 3", arguments.len()));
    }
    let abort = if arguments.len() == 3 {
        if stream_keyword_name("close :abort", &arguments[1])? != "ABORT" {
            return Err(RuntimeError::InvalidForm {
                message: "close accepts only the :abort keyword".to_string(),
                span: None,
            });
        }
        arguments[2].is_truthy()
    } else {
        false
    };
    let stream = stream_reference("close", &arguments[0])?;
    stream
        .borrow_mut()
        .close(abort)
        .map_err(|error| RuntimeError::Io {
            kind: error.kind(),
            message: format!("close: {error}"),
        })?;
    Ok(Value::boolean(true))
}

pub fn streamp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "streamp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Stream(_))))
}

pub fn input_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "input-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_input(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

pub fn output_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "output-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_output(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

pub fn open_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "open-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_open(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

pub fn stream_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stream-element-type", 1)?;
    let stream = stream_reference("stream-element-type", &arguments[0])?;
    if !stream.borrow().is_open() {
        return Err(stream_state_error("stream-element-type", "an open stream"));
    }
    if stream.borrow().element_type_name() == "UNSIGNED-BYTE" {
        Ok(Value::list(vec![Value::symbol("UNSIGNED-BYTE"), Value::Integer(8)]))
    } else {
        Ok(Value::symbol(stream.borrow().element_type_name()))
    }
}

pub fn stream_external_format(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stream-external-format", 1)?;
    let stream = stream_reference("stream-external-format", &arguments[0])?;
    if !stream.borrow().is_open() {
        return Err(stream_state_error("stream-external-format", "an open stream"));
    }
    Ok(Value::keyword("DEFAULT"))
}

pub(crate) fn file_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) { return Err(arity("file-position", "1 or 2", arguments.len())); }
    let stream = stream_reference("file-position", &arguments[0])?;
    let mut stream = stream.borrow_mut();
    if arguments.len() == 1 { return Ok(stream.position().map_or(Value::Nil, |n| Value::Integer(n as i64))); }
    if matches!(arguments[1], Value::Nil) { return Ok(stream.position().map_or(Value::Nil, |n| Value::Integer(n as i64))); }
    let position = index_argument("file-position", &arguments[1])?;
    if stream.set_position(position) { Ok(Value::Integer(position as i64)) }
    else { Err(stream_state_error("file-position", "a valid open file stream position")) }
}

pub(crate) fn file_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-length", 1)?;
    let stream = stream_reference("file-length", &arguments[0])?;
    stream.borrow().length().map_or_else(
        || Err(stream_state_error("file-length", "an open stream")),
        |n| Ok(Value::Integer(n as i64)),
    )
}

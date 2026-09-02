use super::{arity, exact, input_stream_reference, integer_argument, stream_reference, stream_state_error, type_error};
use crate::{RuntimeError, Value};

pub(crate) fn read_byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=4).contains(&arguments.len()) {
        return Err(arity("read-byte", "1 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-byte", arguments.first())?;
    if !stream.borrow().is_input() {
        return Err(stream_state_error("read-byte", "an input stream"));
    }
    if stream.borrow().element_type_name() != "UNSIGNED-BYTE" {
        return Err(stream_state_error("read-byte", "an unsigned-byte stream"));
    }
    let mut stream = stream.borrow_mut();
    let eof_value = arguments.get(1).cloned().unwrap_or(Value::Nil);
    let eof_error_p = arguments.get(2).is_none_or(Value::is_truthy);
    match stream.read_byte() {
        Some(byte) => Ok(Value::Integer(byte as i64)),
        None if eof_error_p => Err(RuntimeError::Io {
            kind: std::io::ErrorKind::UnexpectedEof,
            message: "read-byte reached end of file".to_string(),
        }),
        None => Ok(eof_value),
    }
}

pub(crate) fn write_byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "write-byte", 2)?;
    let byte = integer_argument("write-byte", &arguments[0])?;
    if !(0..=255).contains(&byte) {
        return Err(type_error("write-byte", "an unsigned byte", &arguments[0]));
    }
    let stream = stream_reference("write-byte", &arguments[1])?;
    if !stream.borrow().is_output() {
        return Err(stream_state_error("write-byte", "an output stream"));
    }
    if stream.borrow().element_type_name() != "UNSIGNED-BYTE" {
        return Err(stream_state_error("write-byte", "an unsigned-byte stream"));
    }
    if !stream.borrow_mut().write_byte(byte as u8) {
        return Err(stream_state_error("write-byte", "a byte stream implementation"));
    }
    Ok(arguments[0].clone())
}

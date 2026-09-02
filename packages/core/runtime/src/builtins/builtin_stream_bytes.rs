use super::{arity, exact, input_stream_reference, integer_argument, stream_reference, stream_state_error, type_error};
use crate::{RuntimeError, Value};

pub(crate) fn read_byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-byte", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-byte", arguments.first())?;
    if !stream.borrow().is_input() {
        return Err(stream_state_error("read-byte", "an input stream"));
    }
    if stream.borrow().element_type_name() != "UNSIGNED-BYTE" {
        return Err(stream_state_error("read-byte", "an unsigned-byte stream"));
    }
    Err(stream_state_error("read-byte", "a byte stream implementation"))
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
    Err(stream_state_error("write-byte", "a byte stream implementation"))
}

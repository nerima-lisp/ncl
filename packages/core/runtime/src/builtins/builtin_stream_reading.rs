use super::{
    arity, end_of_file_error, exact, input_stream_reference, peek_character,
    sequence_bounds, stream_reference, stream_state_error, type_error,
};
use crate::{RuntimeError, Value};

pub(crate) fn get_output_stream_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-output-stream-string", 1)?;
    let stream = stream_reference("get-output-stream-string", &arguments[0])?;
    let output = stream
        .borrow_mut()
        .take_output()
        .ok_or_else(|| stream_state_error("get-output-stream-string", "an output stream"))?;
    Ok(Value::string(output))
}

pub(crate) fn read_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-char", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-char", arguments.first())?;
    let eof_error_p = arguments.get(1).is_none_or(Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-char", "an input stream"));
    }
    if stream.element_type_name() != "CHARACTER" {
        return Err(stream_state_error("read-char", "a character stream"));
    }
    match stream.read_char() {
        Some(character) => Ok(Value::Character(character)),
        None if eof_error_p => Err(end_of_file_error("a character")),
        None => Ok(eof_value),
    }
}

pub(crate) fn peek_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 5 {
        return Err(arity("peek-char", "0 to 5", arguments.len()));
    }
    let (peek_type, stream_value, optional_index) =
        if matches!(arguments.first(), Some(Value::Stream(_))) {
            (None, arguments.first(), 1)
        } else {
            (arguments.first(), arguments.get(1), 2)
        };
    let stream = input_stream_reference("peek-char", stream_value)?;
    let eof_error_p = arguments.get(optional_index).is_none_or(Value::is_truthy);
    let eof_value = arguments
        .get(optional_index + 1)
        .cloned()
        .unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("peek-char", "an input stream"));
    }
    match peek_character(&mut stream, peek_type)? {
        Some(character) => Ok(Value::Character(character)),
        None if eof_error_p => Err(end_of_file_error("a character")),
        None => Ok(eof_value),
    }
}

pub(crate) fn unread_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("unread-char", "1 to 2", arguments.len()));
    }
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("unread-char", "a character", value)),
    };
    let stream = input_stream_reference("unread-char", arguments.get(1))?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("unread-char", "an input stream"));
    }
    if !stream.unread_char(character) {
        return Err(stream_state_error(
            "unread-char",
            "the last character read from an open input stream",
        ));
    }
    Ok(Value::Nil)
}

pub(crate) fn listen(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("listen", "0 to 1", arguments.len()));
    }
    let stream = input_stream_reference("listen", arguments.first())?;
    let stream = stream.borrow();
    if !stream.is_input() {
        return Err(stream_state_error("listen", "an input stream"));
    }
    Ok(Value::boolean(stream.peek_char().is_some()))
}

pub(crate) fn read_char_no_hang(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("read-char-no-hang", "0 to 1", arguments.len()));
    }
    let stream = input_stream_reference("read-char-no-hang", arguments.first())?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-char-no-hang", "an input stream"));
    }
    Ok(stream.read_char().map_or(Value::Nil, Value::Character))
}

pub(crate) fn clear_input(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("clear-input", "0 to 1", arguments.len()));
    }
    let stream = input_stream_reference("clear-input", arguments.first())?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("clear-input", "an input stream"));
    }
    while stream.read_char().is_some() {}
    Ok(Value::Nil)
}

pub(crate) fn read_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("read-sequence", "at least 2", arguments.len()));
    }
    let destination = &arguments[0];
    let length = destination
        .sequence_items()
        .map(|items| items.len())
        .ok_or_else(|| type_error("read-sequence", "a vector sequence", destination))?;
    if !matches!(destination, Value::Vector(_) | Value::MutableString(_)) {
        return Err(type_error("read-sequence", "a vector sequence", destination));
    }
    let stream = input_stream_reference("read-sequence", arguments.get(1))?;
    let (start, end) = sequence_bounds("read-sequence", length, &arguments[2..])?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-sequence", "an input stream"));
    }
    let mut index = start;
    while index < end {
        let Some(character) = stream.read_char() else { break };
        match destination {
            Value::Vector(_) => destination
                .set_vector_item(index, Value::Character(character))
                .ok_or_else(|| type_error("read-sequence", "a vector sequence", destination))?,
            Value::MutableString(value) => {
                let mut characters: Vec<char> = value.borrow().chars().collect();
                characters[index] = character;
                *value.borrow_mut() = characters.into_iter().collect();
            }
            _ => unreachable!("read-sequence validates its destination before reading"),
        }
        index += 1;
    }
    Ok(Value::Integer(index as i64))
}

pub(crate) fn read_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-line", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-line", arguments.first())?;
    let eof_error_p = arguments.get(1).is_none_or(Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-line", "an input stream"));
    }
    match stream.read_line() {
        Some((line, eof)) => Ok(Value::values(vec![
            Value::string(line),
            Value::boolean(eof),
        ])),
        None if eof_error_p => Err(end_of_file_error("a line")),
        None => Ok(Value::values(vec![eof_value, Value::boolean(true)])),
    }
}

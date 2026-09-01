use super::{arity, array_option_name, index_argument, sequence_bounds, sequence_elements, stream_state_error, type_error};
use crate::{RuntimeError, Value};

pub(super) fn write_destination(
    function: &str,
    destination: Option<&Value>,
    text: &str,
) -> Result<(), RuntimeError> {
    match destination {
        None | Some(Value::Nil | Value::Boolean(true)) => {
            print!("{text}");
            Ok(())
        }
        Some(Value::Stream(stream)) => {
            if stream.borrow_mut().write(text) {
                Ok(())
            } else {
                Err(stream_state_error(function, "an open output stream"))
            }
        }
        Some(value) => Err(type_error(function, "NIL, T, or an output stream", value)),
    }
}

pub(crate) fn write_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-char", "1 to 2", arguments.len()));
    }
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("write-char", "a character", value)),
    };
    write_destination("write-char", arguments.get(1), &character.to_string())?;
    Ok(Value::Character(character))
}

pub(crate) fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-string", "at least 1", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-string", "a string", value)),
    };
    let (destination, start, end) = write_options("write-string", string, &arguments[1..])?;
    let selected: String = string.chars().skip(start).take(end - start).collect();
    write_destination("write-string", destination.as_ref(), &selected)?;
    Ok(arguments[0].clone())
}

pub(crate) fn write_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-sequence", "at least 1", arguments.len()));
    }
    let elements = sequence_elements("write-sequence", &arguments[0])?;
    let (destination, start, end) = sequence_write_options(&arguments[1..], elements.len())?;
    for element in &elements[start..end] {
        let Value::Character(character) = element else {
            return Err(type_error("write-sequence", "a sequence of characters", &element));
        };
        write_destination("write-sequence", destination.as_ref(), &character.to_string())?;
    }
    Ok(arguments[0].clone())
}

pub(crate) fn terpri(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("terpri", "0 to 1", arguments.len()));
    }
    write_destination("terpri", arguments.first(), "\n")?;
    Ok(Value::Nil)
}

pub(crate) fn fresh_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("fresh-line", "0 to 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil | Value::Boolean(true)) => {
            println!();
            Ok(Value::boolean(true))
        }
        Some(Value::Stream(stream)) => stream
            .borrow_mut()
            .fresh_line()
            .map(Value::boolean)
            .ok_or_else(|| stream_state_error("fresh-line", "an open output stream")),
        Some(value) => Err(type_error(
            "fresh-line",
            "NIL, T, or an output stream",
            value,
        )),
    }
}

pub(crate) fn force_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 { return Err(arity("force-output", "0 to 1", arguments.len())); }
    match arguments.first() {
        None | Some(Value::Nil | Value::Boolean(true)) => Ok(Value::Nil),
        Some(Value::Stream(stream)) => {
            if stream.borrow().is_open() && stream.borrow().is_output() { Ok(Value::Nil) }
            else { Err(stream_state_error("force-output", "an open output stream")) }
        }
        Some(value) => Err(type_error("force-output", "NIL, T, or an output stream", value)),
    }
}

pub(crate) fn finish_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    force_output(arguments)
}

pub(crate) fn clear_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 { return Err(arity("clear-output", "0 to 1", arguments.len())); }
    match arguments.first() {
        None | Some(Value::Nil | Value::Boolean(true)) => Ok(Value::Nil),
        Some(Value::Stream(stream)) => {
            if stream.borrow_mut().clear_output() { Ok(Value::Nil) }
            else { Err(stream_state_error("clear-output", "an open output stream")) }
        }
        Some(value) => Err(type_error("clear-output", "NIL, T, or an output stream", value)),
    }
}

pub(crate) fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-line", "at least 1", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-line", "a string", value)),
    };
    let (destination, start, end) = write_options("write-line", string, &arguments[1..])?;
    let selected: String = string.chars().skip(start).take(end - start).collect();
    let mut line = String::with_capacity(selected.len() + 1);
    line.push_str(&selected);
    line.push('\n');
    write_destination("write-line", destination.as_ref(), &line)?;
    Ok(arguments[0].clone())
}

fn write_options(
    function: &str,
    string: &str,
    arguments: &[Value],
) -> Result<(Option<Value>, usize, usize), RuntimeError> {
    let (destination, options) = match arguments.first() {
        Some(Value::Stream(_) | Value::Nil | Value::Boolean(true)) => {
            (Some(arguments[0].clone()), &arguments[1..])
        }
        Some(Value::Keyword(_)) => (None, arguments),
        Some(value) => return Err(type_error(function, "NIL, T, or an output stream", value)),
        None => (None, arguments),
    };
    if !options.len().is_multiple_of(2) {
        return Err(arity(function, "keyword/value pairs", options.len()));
    }
    let mut start = 0;
    let mut end = string.chars().count();
    for pair in options.as_chunks::<2>().0 {
        match array_option_name(function, &pair[0])?.as_str() {
            "START" => start = index_argument(function, &pair[1])?,
            "END" => end = index_argument(function, &pair[1])?,
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > string.chars().count() {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bounds are invalid"),
            span: None,
        });
    }
    Ok((destination, start, end))
}

fn sequence_write_options(
    arguments: &[Value], length: usize,
) -> Result<(Option<Value>, usize, usize), RuntimeError> {
    let (destination, options) = match arguments.first() {
        Some(Value::Stream(_) | Value::Nil | Value::Boolean(true)) => {
            (Some(arguments[0].clone()), &arguments[1..])
        }
        Some(Value::Keyword(_)) | None => (None, arguments),
        Some(value) => return Err(type_error("write-sequence", "NIL, T, or an output stream", value)),
    };
    if !options.len().is_multiple_of(2) {
        return Err(arity("write-sequence", "keyword/value pairs", options.len()));
    }
    let (start, end) = sequence_bounds("write-sequence", length, options)?;
    Ok((destination, start, end))
}

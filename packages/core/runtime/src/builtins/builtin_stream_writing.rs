use std::cell::RefCell;
use std::rc::Rc;

use super::{arity, integer_value, sequence_items, stream_bound, stream_state_error, type_error};
use crate::{RuntimeError, Stream, Value};

fn output_stream_reference(
    function: &str,
    value: Option<&Value>,
) -> Result<Rc<RefCell<Stream>>, RuntimeError> {
    let stream = match value {
        None | Some(Value::Nil | Value::Boolean(true)) => super::standard_output(),
        Some(Value::Stream(stream)) => Some(Rc::clone(stream)),
        Some(value) => {
            return Err(type_error(function, "NIL, T, or an output stream", value));
        }
    }
    .ok_or_else(|| stream_state_error(function, "an open output stream"))?;
    if !stream.borrow().is_open() || !stream.borrow().is_output() {
        return Err(stream_state_error(function, "an open output stream"));
    }
    Ok(stream)
}

fn flush_output_stream(function: &str, stream: Rc<RefCell<Stream>>) -> Result<(), RuntimeError> {
    stream
        .borrow_mut()
        .flush_output()
        .map_err(|error| RuntimeError::Io {
            kind: error.kind(),
            message: format!("{function}: {error}"),
        })
}

pub(crate) fn force_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("force-output", "0 to 1", arguments.len()));
    }
    let stream = output_stream_reference("force-output", arguments.first())?;
    flush_output_stream("force-output", stream)?;
    Ok(Value::Nil)
}

pub(crate) fn finish_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("finish-output", "0 to 1", arguments.len()));
    }
    let stream = output_stream_reference("finish-output", arguments.first())?;
    flush_output_stream("finish-output", stream)?;
    Ok(Value::Nil)
}

pub(crate) fn clear_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("clear-output", "0 to 1", arguments.len()));
    }
    let stream = output_stream_reference("clear-output", arguments.first())?;
    if !stream.borrow_mut().clear_output() {
        return Err(stream_state_error("clear-output", "an open output stream"));
    }
    Ok(Value::Nil)
}

fn keyword_name(value: &Value) -> Option<&str> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Some(name.as_ref()),
        Value::InternedSymbol(symbol) if symbol.keyword() => Some(symbol.name()),
        _ => None,
    }
}

fn string_write_options(
    function: &str,
    arguments: &[Value],
) -> Result<(Option<usize>, usize, usize), RuntimeError> {
    if !(1..=6).contains(&arguments.len()) {
        return Err(arity(function, "1 to 6", arguments.len()));
    }
    let length = match &arguments[0] {
        Value::String(value) => value.chars().count(),
        value => return Err(type_error(function, "a string", value)),
    };
    let (destination, keyword_start) = match arguments.get(1) {
        Some(value) if keyword_name(value).is_some() => (None, 1),
        Some(_) => (Some(1), 2),
        None => (None, 1),
    };
    let keyword_arguments = &arguments[keyword_start..];
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} requires keyword/value pairs"),
            span: None,
        });
    }
    let mut start = 0;
    let mut end = length;
    for pair in keyword_arguments.chunks_exact(2) {
        let name = keyword_name(&pair[0]).ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("{function} requires keyword/value pairs"),
            span: None,
        })?;
        match name.to_ascii_uppercase().as_str() {
            "START" => start = stream_bound(function, &pair[1], length)?,
            "END" => end = stream_bound(function, &pair[1], length)?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} start must not exceed end"),
            span: None,
        });
    }
    Ok((destination, start, end))
}

fn selected_string(string: &str, start: usize, end: usize) -> String {
    string.chars().skip(start).take(end - start).collect()
}

fn write_to_standard_output(function: &str, text: &str) -> Result<(), RuntimeError> {
    if let Some(stream) = super::standard_output() {
        if stream.borrow_mut().write(text) {
            return Ok(());
        }
        return Err(stream_state_error(function, "an open output stream"));
    }
    print!("{text}");
    Ok(())
}

pub(crate) fn write_destination(
    function: &str,
    destination: Option<&Value>,
    text: &str,
) -> Result<(), RuntimeError> {
    match destination {
        None | Some(Value::Nil | Value::Boolean(true)) => write_to_standard_output(function, text),
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

pub(crate) fn write_byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-byte", "1 to 2", arguments.len()));
    }
    let integer = integer_value("write-byte", &arguments[0])?;
    let byte = u8::try_from(&integer).map_err(|_| RuntimeError::InvalidForm {
        message: "write-byte requires an integer between 0 and 255".to_string(),
        span: None,
    })?;
    let stream = match arguments.get(1) {
        None | Some(Value::Nil | Value::Boolean(true)) => super::standard_output(),
        Some(Value::Stream(stream)) => Some(std::rc::Rc::clone(stream)),
        Some(value) => {
            return Err(type_error(
                "write-byte",
                "NIL, T, or a binary output stream",
                value,
            ));
        }
    };
    let Some(stream) = stream else {
        return Err(stream_state_error(
            "write-byte",
            "an open binary output stream",
        ));
    };
    if !stream.borrow().is_binary_output() {
        return Err(stream_state_error(
            "write-byte",
            "an open binary output stream",
        ));
    }
    if !stream.borrow_mut().write_byte(byte) {
        return Err(stream_state_error(
            "write-byte",
            "an open binary output stream",
        ));
    }
    Ok(Value::Integer(i64::from(byte)))
}

pub(crate) fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (destination, start, end) = string_write_options("write-string", arguments)?;
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-string", "a string", value)),
    };
    let text = selected_string(string, start, end);
    write_destination(
        "write-string",
        destination.and_then(|index| arguments.get(index)),
        &text,
    )?;
    Ok(arguments[0].clone())
}

pub(crate) fn write_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=6).contains(&arguments.len()) {
        return Err(arity("write-sequence", "1 to 6", arguments.len()));
    }
    let items = sequence_items(&arguments[0])
        .ok_or_else(|| type_error("write-sequence", "a sequence", &arguments[0]))?;
    let (destination, keyword_start) = match arguments.get(1) {
        Some(value) if keyword_name(value).is_some() => (None, 1),
        Some(_) => (Some(1), 2),
        None => (None, 1),
    };
    let keyword_arguments = &arguments[keyword_start..];
    if !keyword_arguments.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "write-sequence requires keyword/value pairs".to_string(),
            span: None,
        });
    }
    let mut start = 0;
    let mut end = items.len();
    for pair in keyword_arguments.chunks_exact(2) {
        let name = keyword_name(&pair[0]).ok_or_else(|| RuntimeError::InvalidForm {
            message: "write-sequence requires keyword/value pairs".to_string(),
            span: None,
        })?;
        match name.to_ascii_uppercase().as_str() {
            "START" => start = stream_bound("write-sequence", &pair[1], items.len())?,
            "END" => end = stream_bound("write-sequence", &pair[1], items.len())?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("write-sequence does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "write-sequence start must not exceed end".to_string(),
            span: None,
        });
    }
    let selected = &items[start..end];
    if selected
        .iter()
        .all(|value| matches!(value, Value::Character(_)))
    {
        let text: String = selected
            .iter()
            .map(|value| match value {
                Value::Character(character) => *character,
                _ => unreachable!(),
            })
            .collect();
        write_destination(
            "write-sequence",
            destination.and_then(|index| arguments.get(index)),
            &text,
        )?;
        return Ok(arguments[0].clone());
    }
    if !selected
        .iter()
        .all(|value| integer_value("write-sequence", value).is_ok())
    {
        return Err(type_error(
            "write-sequence",
            "a sequence of characters or unsigned bytes",
            &arguments[0],
        ));
    }
    let mut byte_arguments = Vec::with_capacity(2);
    for value in selected {
        byte_arguments.clear();
        byte_arguments.push(value.clone());
        if let Some(index) = destination {
            byte_arguments.push(arguments[index].clone());
        }
        write_byte(&byte_arguments)?;
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
            if let Some(stream) = super::standard_output() {
                return stream
                    .borrow_mut()
                    .fresh_line()
                    .map(Value::boolean)
                    .ok_or_else(|| stream_state_error("fresh-line", "an open output stream"));
            }
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

pub(crate) fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (destination, start, end) = string_write_options("write-line", arguments)?;
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-line", "a string", value)),
    };
    let selected = selected_string(string, start, end);
    let mut line = String::with_capacity(selected.len() + 1);
    line.push_str(&selected);
    line.push('\n');
    write_destination(
        "write-line",
        destination.and_then(|index| arguments.get(index)),
        &line,
    )?;
    Ok(arguments[0].clone())
}

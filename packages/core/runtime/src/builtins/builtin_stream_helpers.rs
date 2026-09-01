use std::cell::RefCell;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Span};

use super::type_error;
use crate::{RuntimeError, Stream, Value};

pub(super) fn stream_reference<'a>(
    function: &str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Value::Stream(stream) => Ok(stream),
        value => Err(type_error(function, "a stream", value)),
    }
}

pub(super) fn input_stream_reference(
    function: &str,
    value: Option<&Value>,
) -> Result<Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Some(Value::Stream(stream)) => Ok(stream.clone()),
        None | Some(Value::Nil | Value::Boolean(true)) => {
            let stream = super::standard_streams::input().ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} requires an explicit input stream; standard input is unavailable"),
                span: None,
            })?;
            match stream { Value::Stream(stream) => Ok(stream), value => Err(type_error(function, "an input stream", &value)) }
        }
        Some(value) => Err(type_error(function, "an input stream", value)),
    }
}

pub(super) fn stream_state_error(function: &str, expected: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} requires {expected}"),
        span: None,
    }
}

pub(super) fn end_of_file_error(context: &'static str) -> RuntimeError {
    RuntimeError::Read(Box::new(ReadError::new(
        ReadErrorKind::UnexpectedEnd { context },
        Span::new(0, 0),
    )))
}

pub(super) fn peek_character(
    stream: &mut Stream,
    peek_type: Option<&Value>,
) -> Result<Option<char>, RuntimeError> {
    match peek_type {
        None | Some(Value::Nil | Value::Boolean(false | true) | Value::Character(_)) => {}
        Some(value) => return Err(type_error("peek-char", "NIL, T, or a character", value)),
    }

    loop {
        let Some(character) = stream.peek_char() else {
            return Ok(None);
        };
        let matches = match peek_type {
            None | Some(Value::Nil | Value::Boolean(false)) => true,
            Some(Value::Boolean(true)) => !character.is_whitespace(),
            Some(Value::Character(expected)) => character == *expected,
            Some(_) => unreachable!("peek-char type was validated above"),
        };
        if matches {
            return Ok(Some(character));
        }
        let _ = stream.read_char();
    }
}

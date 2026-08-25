use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Reader, Span};

use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Function, Rational, RuntimeError, Stream, Value};

mod arrays_hashes;
mod characters_strings;
mod constants;
mod errors;
mod files;
mod format;
mod lists;
mod numeric;
mod numeric_helpers;
mod parse_integer;
mod predicates;
mod property_lists;
mod registry;
mod sequence_primitives;
mod sequences;

pub(crate) use errors::{arity, exact, type_error};
pub(crate) use files::{
    delete_file, file_write_date, open_file, probe_file, rename_file, truename,
};
mod symbol_conditions;
mod type_system;

use constants::install_constants;

pub(crate) use arrays_hashes::*;
pub(crate) use characters_strings::*;
pub(crate) use format::*;
pub(crate) use lists::*;
pub(crate) use numeric::*;
pub(crate) use numeric_helpers::*;
pub(crate) use parse_integer::*;
pub(crate) use predicates::*;
pub(crate) use property_lists::*;
pub(crate) use sequence_primitives::*;
pub(crate) use sequences::*;
pub(crate) use symbol_conditions::*;
pub(crate) use type_system::*;

pub fn install(environment: &Environment) {
    registry::install_functions(environment);
    registry::install_primitives(environment);
    install_constants(environment);
}

fn print_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("print", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("print", arguments.get(1), "\n")?;
    write_destination("print", arguments.get(1), &text)?;
    write_destination("print", arguments.get(1), "\n")?;
    Ok(arguments[0].clone())
}

fn princ(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("princ", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], false);
    write_destination("princ", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

fn prin1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("prin1", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("prin1", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

fn write_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write", "at least 1", arguments.len()));
    }
    let (escape, stream) = parse_print_options("write", &arguments[1..], true)?;
    let text = printed_value(&arguments[0], escape);
    write_destination("write", stream.as_ref(), &text)?;
    Ok(arguments[0].clone())
}

fn write_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-to-string", "at least 1", arguments.len()));
    }
    let (escape, _) = parse_print_options("write-to-string", &arguments[1..], false)?;
    Ok(Value::string(printed_value(&arguments[0], escape)))
}

fn parse_print_options(
    function: &str,
    options: &[Value],
    allow_stream: bool,
) -> Result<(bool, Option<Value>), RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} requires keyword/value pairs"),
            span: None,
        });
    }
    let mut escape = true;
    let mut stream = None;
    for pair in options.chunks_exact(2) {
        let name = array_option_name(function, &pair[0])?;
        match name.as_str() {
            "ESCAPE" => escape = pair[1].is_truthy(),
            "STREAM" if allow_stream => stream = Some(pair[1].clone()),
            "STREAM" => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :stream"),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok((escape, stream))
}

fn printed_value(value: &Value, escape: bool) -> String {
    match value {
        Value::String(value) if !escape => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::List(values) => {
            let contents = values
                .iter()
                .map(|value| printed_value(value, escape))
                .collect::<Vec<_>>()
                .join(" ");
            format!("({contents})")
        }
        Value::DottedList { items, tail } => {
            let mut text = String::from("(");
            if !items.is_empty() {
                text.push_str(
                    &items
                        .iter()
                        .map(|value| printed_value(value, escape))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                text.push(' ');
            }
            text.push_str(". ");
            text.push_str(&printed_value(tail, escape));
            text.push(')');
            text
        }
        Value::Vector(values) => {
            let contents = values
                .iter()
                .map(|value| printed_value(value, escape))
                .collect::<Vec<_>>()
                .join(" ");
            format!("#({contents})")
        }
        _ => value.to_string(),
    }
}

fn read_from_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("read-from-string", "at least 1", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("read-from-string", "a string", value)),
    };
    let eof_error_p = arguments.get(1).is_none_or(Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let source_length = source.chars().count();
    let mut start = 0;
    let mut end = source_length;
    let mut preserving_whitespace = false;
    let keyword_arguments = arguments.get(3..).unwrap_or_default();
    if keyword_arguments.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "read-from-string keyword arguments must be name/value pairs".to_string(),
            span: None,
        });
    }
    for pair in keyword_arguments.chunks_exact(2) {
        let name = match &pair[0] {
            Value::Keyword(name) | Value::KeywordExact(name) => name.as_ref(),
            value => return Err(type_error("read-from-string", "a keyword", value)),
        };
        if name.eq_ignore_ascii_case("START") {
            start = stream_bound("read-from-string", &pair[1], source_length)?;
        } else if name.eq_ignore_ascii_case("END") {
            end = stream_bound("read-from-string", &pair[1], source_length)?;
        } else if name.eq_ignore_ascii_case("PRESERVE-WHITESPACE") {
            preserving_whitespace = pair[1].is_truthy();
        } else {
            return Err(RuntimeError::InvalidForm {
                message: format!("read-from-string does not support keyword :{name}"),
                span: None,
            });
        }
    }
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "read-from-string start must not exceed end".to_string(),
            span: None,
        });
    }
    let window = source
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    let mut reader = Reader::new(&window);
    let (value, byte_position) = match reader.read_form()? {
        Some(form) => {
            let value = quoted_form_value(&form)?;
            let byte_position = if preserving_whitespace {
                form.span.end
            } else {
                reader.consume_one_whitespace_after_form();
                reader.position()
            };
            (value, byte_position)
        }
        None => {
            let position = reader.position();
            if eof_error_p {
                return Err(RuntimeError::Read(ReadError::new(
                    ReadErrorKind::UnexpectedEnd { context: "a form" },
                    Span::new(position, position),
                )));
            }
            (eof_value, position)
        }
    };
    let local_position = window[..byte_position].chars().count();
    let position = start
        .checked_add(local_position)
        .ok_or(RuntimeError::NumericOverflow)?;
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![value, Value::Integer(position)]))
}

fn read(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_stream_form("read", arguments, false)
}

fn read_preserving_whitespace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_stream_form("read-preserving-whitespace", arguments, true)
}

fn read_stream_form(
    function: &str,
    arguments: &[Value],
    preserving_whitespace: bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity(function, "0 to 4", arguments.len()));
    }
    let stream = match arguments.first() {
        Some(Value::Stream(stream)) => stream,
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "{function} requires an explicit input stream; standard input is unavailable"
                ),
                span: None,
            });
        }
        Some(value) => return Err(type_error(function, "an input stream", value)),
    };
    let eof_error_p = arguments.get(1).is_none_or(Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let source = {
        let stream = stream.borrow();
        if !stream.is_input() {
            return Err(stream_state_error(function, "an input stream"));
        }
        stream
            .remaining_input()
            .ok_or_else(|| stream_state_error(function, "an open input stream"))?
    };
    let mut reader = Reader::new(&source);
    let (value, byte_position) = match reader.read_form()? {
        Some(form) => {
            let value = quoted_form_value(&form)?;
            let byte_position = if preserving_whitespace {
                form.span.end
            } else {
                reader.consume_one_whitespace_after_form();
                reader.position()
            };
            (value, byte_position)
        }
        None => {
            let position = reader.position();
            let consumed = source[..position].chars().count();
            if !stream.borrow_mut().consume_input(consumed) {
                return Err(stream_state_error(function, "an open input stream"));
            }
            if eof_error_p {
                return Err(RuntimeError::Read(ReadError::new(
                    ReadErrorKind::UnexpectedEnd { context: "a form" },
                    Span::new(position, position),
                )));
            }
            return Ok(eof_value);
        }
    };
    let consumed = source[..byte_position].chars().count();
    if !stream.borrow_mut().consume_input(consumed) {
        return Err(stream_state_error(function, "an open input stream"));
    }
    Ok(value)
}

fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn stream_bound(function: &str, value: &Value, length: usize) -> Result<usize, RuntimeError> {
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

fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-string-output-stream", 0)?;
    Ok(Value::string_output_stream())
}

fn stream_keyword_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Ok(normalize_name(name)),
        value => Err(type_error(function, "a keyword", value)),
    }
}

fn stream_reference<'a>(
    function: &str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Value::Stream(stream) => Ok(stream),
        value => Err(type_error(function, "a stream", value)),
    }
}

fn input_stream_reference<'a>(
    function: &str,
    value: Option<&'a Value>,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Some(Value::Stream(stream)) => Ok(stream),
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} requires an explicit input stream; standard input is unavailable"
            ),
            span: None,
        }),
        Some(value) => Err(type_error(function, "an input stream", value)),
    }
}

fn stream_state_error(function: &str, expected: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} requires {expected}"),
        span: None,
    }
}

fn end_of_file_error(context: &'static str) -> RuntimeError {
    RuntimeError::Read(ReadError::new(
        ReadErrorKind::UnexpectedEnd { context },
        Span::new(0, 0),
    ))
}

fn peek_character(
    stream: &mut Stream,
    peek_type: Option<&Value>,
) -> Result<Option<char>, RuntimeError> {
    match peek_type {
        None
        | Some(Value::Nil)
        | Some(Value::Boolean(false))
        | Some(Value::Boolean(true))
        | Some(Value::Character(_)) => {}
        Some(value) => return Err(type_error("peek-char", "NIL, T, or a character", value)),
    }

    loop {
        let Some(character) = stream.peek_char() else {
            return Ok(None);
        };
        let matches = match peek_type {
            None | Some(Value::Nil) | Some(Value::Boolean(false)) => true,
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

fn get_output_stream_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-output-stream-string", 1)?;
    let stream = stream_reference("get-output-stream-string", &arguments[0])?;
    let output = stream
        .borrow_mut()
        .take_output()
        .ok_or_else(|| stream_state_error("get-output-stream-string", "an output stream"))?;
    Ok(Value::string(output))
}

fn read_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    match stream.read_char() {
        Some(character) => Ok(Value::Character(character)),
        None if eof_error_p => Err(end_of_file_error("a character")),
        None => Ok(eof_value),
    }
}

fn peek_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn unread_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn read_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn write_destination(
    function: &str,
    destination: Option<&Value>,
    text: &str,
) -> Result<(), RuntimeError> {
    match destination {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
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

fn write_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-string", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-string", "a string", value)),
    };
    write_destination("write-string", arguments.get(1), string)?;
    Ok(arguments[0].clone())
}

fn terpri(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("terpri", "0 to 1", arguments.len()));
    }
    write_destination("terpri", arguments.first(), "\n")?;
    Ok(Value::Nil)
}

fn fresh_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("fresh-line", "0 to 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
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

fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-line", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-line", "a string", value)),
    };
    let mut line = String::with_capacity(string.len() + 1);
    line.push_str(string);
    line.push('\n');
    write_destination("write-line", arguments.get(1), &line)?;
    Ok(arguments[0].clone())
}

fn close_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
        .map_err(|error| RuntimeError::Io(format!("close: {error}")))?;
    Ok(Value::boolean(true))
}

fn streamp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "streamp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Stream(_))))
}

fn input_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "input-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_input(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

fn output_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "output-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_output(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

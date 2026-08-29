use std::cell::RefCell;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Span};

use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Rational, RuntimeError, Stream, Value};

#[cfg(test)]
mod file_tests;

mod builtin_integer;
use builtin_integer::parse_integer;

mod builtin_characters;
use builtin_characters::{
    alpha_character_p, alphanumeric_p, both_case_p, char_code, char_int, character,
    character_case_equal, character_case_greater_equal, character_case_greater_than,
    character_case_less_equal, character_case_less_than, character_case_not_equal,
    character_downcase, character_equal, character_greater_equal, character_greater_than,
    character_less_equal, character_less_than, character_name, character_not_equal,
    character_upcase, character_value, code_char, digit_character, digit_character_p,
    graphic_character_p, int_char, lower_case_p, make_string, name_character, simple_character,
    standard_character_p, string_value, upper_case_p,
};

mod builtin_arrays;
use builtin_arrays::{
    aref, array_dimension, array_dimensions, array_element_type, array_in_bounds_p, array_rank,
    array_row_major_index, array_total_size, arrayp, bit, make_array, row_major_aref,
    simple_array_p, svref, vector,
};

mod builtin_helpers;
use builtin_helpers::{arity, exact, number_error, type_error};

mod builtin_reading;
use builtin_reading::{read, read_from_string, read_preserving_whitespace};

mod builtin_hash_tables;
pub use builtin_hash_tables::hash_table_key_equal;
#[allow(clippy::wildcard_imports)]
use builtin_hash_tables::*;

mod builtin_array_helpers;
#[allow(clippy::wildcard_imports)]
use builtin_array_helpers::*;

mod builtin_stream_predicates;
use builtin_stream_predicates::{close_stream, input_stream_p, output_stream_p, streamp};

mod builtin_random;
use builtin_random::{make_random_state, random, random_state_p};

mod builtin_format_data;
use builtin_format_data::{ENGLISH_NUMBER_GROUPS, FORMAT_DIGITS};

mod registry;
pub use registry::install;
mod builtin_numeric_ops;
#[allow(clippy::wildcard_imports)]
pub use builtin_numeric_ops::*;
mod builtin_sequences;
#[allow(clippy::wildcard_imports)]
pub use builtin_sequences::*;
mod builtin_list_ops;
#[allow(clippy::wildcard_imports)]
pub use builtin_list_ops::*;
mod types;
#[allow(clippy::wildcard_imports)]
pub use types::*;

#[cfg(test)]
mod builtins_tests;

pub mod type_predicates;
pub use type_predicates::eql_value;
#[allow(clippy::wildcard_imports)]
use type_predicates::*;

fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or_else(|| arguments[0].type_name()),
    ))
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
    for pair in options.as_chunks::<2>().0 {
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
        Value::List(values) => delimited_values(values, "(", ")", escape),
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
        Value::Vector(values) => delimited_values(values, "#(", ")", escape),
        _ => value.to_string(),
    }
}

fn delimited_values(values: &[Value], opening: &str, closing: &str, escape: bool) -> String {
    let contents = values
        .iter()
        .map(|value| printed_value(value, escape))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{opening}{contents}{closing}")
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

fn pathname_argument(function: &str, value: &Value) -> Result<PathBuf, RuntimeError> {
    match value {
        Value::String(value) => Ok(PathBuf::from(value.as_ref())),
        value => Err(type_error(function, "a string pathname", value)),
    }
}

fn open_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("open", "at least 1", arguments.len()));
    }
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "open requires keyword/value pairs after the pathname".to_string(),
            span: None,
        });
    }
    let path = pathname_argument("open", &arguments[0])?;
    let mut direction = "INPUT".to_string();
    let mut if_does_not_exist = None;
    let mut if_exists = None;
    for pair in arguments[1..].as_chunks::<2>().0 {
        let keyword = stream_keyword_name("open", &pair[0])?;
        match keyword.as_str() {
            "DIRECTION" => {
                direction = stream_keyword_name("open :direction", &pair[1])?;
            }
            "IF-DOES-NOT-EXIST" => {
                if_does_not_exist = Some(stream_keyword_name("open :if-does-not-exist", &pair[1])?);
            }
            "IF-EXISTS" => {
                if_exists = Some(stream_keyword_name("open :if-exists", &pair[1])?);
            }
            "ELEMENT-TYPE" | "EXTERNAL-FORMAT" => {}
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open does not recognize keyword :{keyword}"),
                    span: None,
                });
            }
        }
    }

    let if_does_not_exist = if_does_not_exist.unwrap_or_else(|| {
        if direction == "INPUT" || direction == "IO" {
            "ERROR".to_string()
        } else {
            "CREATE".to_string()
        }
    });
    let if_exists = if_exists.unwrap_or_else(|| "NEW-VERSION".to_string());
    match direction.as_str() {
        "INPUT" => open_input_file(&path, &if_does_not_exist),
        "OUTPUT" => open_output_file(&path, &if_does_not_exist, &if_exists),
        "PROBE" => {
            if path.exists() {
                Ok(Value::file_input_stream(""))
            } else {
                Ok(Value::Nil)
            }
        }
        "IO" => open_io_file(&path, &if_does_not_exist, &if_exists),
        _ => Err(RuntimeError::InvalidForm {
            message: format!("open received unknown direction :{direction}"),
            span: None,
        }),
    }
}

fn probe_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "probe-file", 1)?;
    let path = pathname_argument("probe-file", &arguments[0])?;
    match std::fs::metadata(&path) {
        Ok(_) => Ok(arguments[0].clone()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        Err(error) => Err(RuntimeError::Io(format!(
            "probe-file {}: {error}",
            path.display()
        ))),
    }
}

fn delete_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "delete-file", 1)?;
    let path = pathname_argument("delete-file", &arguments[0])?;
    std::fs::remove_file(&path)
        .map_err(|error| RuntimeError::Io(format!("delete-file {}: {error}", path.display())))?;
    Ok(Value::boolean(true))
}

fn rename_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rename-file", 2)?;
    let old_path = pathname_argument("rename-file", &arguments[0])?;
    let new_path = pathname_argument("rename-file", &arguments[1])?;
    let old_truename = std::fs::canonicalize(&old_path).map_err(|error| {
        RuntimeError::Io(format!("rename-file {}: {error}", old_path.display()))
    })?;
    std::fs::rename(&old_path, &new_path).map_err(|error| {
        RuntimeError::Io(format!(
            "rename-file {} to {}: {error}",
            old_path.display(),
            new_path.display()
        ))
    })?;
    let new_truename = std::fs::canonicalize(&new_path).map_err(|error| {
        RuntimeError::Io(format!("rename-file {}: {error}", new_path.display()))
    })?;
    Ok(Value::values(vec![
        arguments[1].clone(),
        Value::string(old_truename.to_string_lossy().to_string()),
        Value::string(new_truename.to_string_lossy().to_string()),
    ]))
}

fn file_write_date(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-write-date", 1)?;
    let path = pathname_argument("file-write-date", &arguments[0])?;
    let modified = std::fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| {
            RuntimeError::Io(format!("file-write-date {}: {error}", path.display()))
        })?;
    let seconds_since_unix = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            RuntimeError::Io(format!("file-write-date {}: {error}", path.display()))
        })?;
    let seconds_since_unix = i64::try_from(seconds_since_unix.as_secs()).map_err(|_| {
        RuntimeError::Io(format!(
            "file-write-date {}: modification time is out of range",
            path.display()
        ))
    })?;
    let universal_time = seconds_since_unix
        .checked_add(2_208_988_800)
        .ok_or_else(|| {
            RuntimeError::Io(format!(
                "file-write-date {}: modification time is out of range",
                path.display()
            ))
        })?;
    Ok(Value::Integer(universal_time))
}

fn truename(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "truename", 1)?;
    let path = pathname_argument("truename", &arguments[0])?;
    let canonical = std::fs::canonicalize(&path)
        .map_err(|error| RuntimeError::Io(format!("truename {}: {error}", path.display())))?;
    Ok(Value::string(canonical.to_string_lossy().to_string()))
}

fn stream_keyword_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Ok(normalize_name(name)),
        value => Err(type_error(function, "a keyword", value)),
    }
}

fn open_input_file(path: &std::path::Path, if_does_not_exist: &str) -> Result<Value, RuntimeError> {
    if !path.exists() {
        match if_does_not_exist {
            "NIL" => return Ok(Value::Nil),
            "CREATE" => {
                std::fs::write(path, []).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?;
            }
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    }
    let source = std::fs::read_to_string(path)
        .map_err(|error| RuntimeError::Io(format!("open {}: {error}", path.display())))?;
    Ok(Value::file_input_stream(&source))
}

fn open_output_file(
    path: &std::path::Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file already exists",
                    path.display()
                )));
            }
            "APPEND" => {
                let source = std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?;
                return Ok(Value::file_output_stream(path.to_path_buf(), source));
            }
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {}
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open received unknown :if-exists value :{if_exists}"),
                    span: None,
                });
            }
        }
    } else {
        match if_does_not_exist {
            "CREATE" => {}
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    }
    Ok(Value::file_output_stream(path.to_path_buf(), String::new()))
}

fn open_io_file(
    path: &std::path::Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut append = false;
    let source = if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file already exists",
                    path.display()
                )));
            }
            "APPEND" => {
                append = true;
                std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?
            }
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {
                std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open received unknown :if-exists value :{if_exists}"),
                    span: None,
                });
            }
        }
    } else {
        match if_does_not_exist {
            "CREATE" => String::new(),
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    };
    Ok(Value::file_io_stream(path.to_path_buf(), &source, append))
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
        None | Some(Value::Nil | Value::Boolean(true)) => Err(RuntimeError::InvalidForm {
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
    RuntimeError::Read(Box::new(ReadError::new(
        ReadErrorKind::UnexpectedEnd { context },
        Span::new(0, 0),
    )))
}

fn peek_character(
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

mod format;
pub use format::format_control;
use format::format_value;
mod numbers;
#[allow(clippy::wildcard_imports)]
use numbers::*;

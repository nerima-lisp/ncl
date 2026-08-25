use super::*;
use std::io::Write as _;

pub(super) fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn stream_bound(
    function: &str,
    value: &Value,
    length: usize,
) -> Result<usize, RuntimeError> {
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

pub(super) fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() % 2 != 0 {
        return Err(arity(
            "make-string-output-stream",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut element_type = None;
    for pair in arguments.chunks_exact(2) {
        let keyword = stream_keyword_name("make-string-output-stream", &pair[0])?;
        if keyword != "ELEMENT-TYPE" {
            return Err(RuntimeError::InvalidForm {
                message: format!("make-string-output-stream does not support keyword :{keyword}"),
                span: None,
            });
        }
        if element_type.is_some() {
            return Err(RuntimeError::InvalidForm {
                message: "make-string-output-stream received duplicate :element-type".to_string(),
                span: None,
            });
        }
        element_type = Some(&pair[1]);
    }
    if let Some(element_type) = element_type {
        let element_type = type_designator_name("make-string-output-stream", element_type)?;
        if element_type != "CHARACTER" {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "make-string-output-stream only supports :element-type CHARACTER, got {element_type}"
                ),
                span: None,
            });
        }
    }
    Ok(Value::string_output_stream())
}

pub(super) fn string_input_stream_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL-STRING-INPUT-STREAM-POSITION", 1)?;
    let stream = stream_reference("__NCL-STRING-INPUT-STREAM-POSITION", &arguments[0])?;
    let stream = stream.borrow();
    let position = stream.string_input_position().ok_or_else(|| {
        stream_state_error(
            "__NCL-STRING-INPUT-STREAM-POSITION",
            "an open string input stream",
        )
    })?;
    Ok(Value::Integer(position as i64))
}

pub(super) fn make_two_way_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-two-way-stream", 2)?;
    let input = stream_argument("make-two-way-stream", &arguments[0], true)?;
    let output = stream_argument("make-two-way-stream", &arguments[1], false)?;
    Ok(Value::two_way_stream(input, output))
}

pub(super) fn two_way_stream_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "two-way-stream-input-stream", 1)?;
    let stream = stream_reference("two-way-stream-input-stream", &arguments[0])?;
    let input = stream
        .borrow()
        .two_way_input_stream()
        .ok_or_else(|| stream_state_error("two-way-stream-input-stream", "a two-way stream"))?;
    Ok(Value::Stream(input))
}

pub(super) fn two_way_stream_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "two-way-stream-output-stream", 1)?;
    let stream = stream_reference("two-way-stream-output-stream", &arguments[0])?;
    let output = stream
        .borrow()
        .two_way_output_stream()
        .ok_or_else(|| stream_state_error("two-way-stream-output-stream", "a two-way stream"))?;
    Ok(Value::Stream(output))
}

pub(super) fn make_broadcast_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let streams = stream_arguments("make-broadcast-stream", arguments, false)?;
    Ok(Value::broadcast_stream(streams))
}

pub(super) fn broadcast_stream_streams(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "broadcast-stream-streams", 1)?;
    let stream = stream_reference("broadcast-stream-streams", &arguments[0])?;
    let streams = stream
        .borrow()
        .broadcast_streams()
        .ok_or_else(|| stream_state_error("broadcast-stream-streams", "a broadcast stream"))?;
    Ok(Value::list(
        streams.into_iter().map(Value::Stream).collect(),
    ))
}

pub(super) fn concatenated_stream_streams(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "concatenated-stream-streams", 1)?;
    let stream = stream_reference("concatenated-stream-streams", &arguments[0])?;
    let streams = stream.borrow().concatenated_streams().ok_or_else(|| {
        stream_state_error("concatenated-stream-streams", "a concatenated stream")
    })?;
    Ok(Value::list(
        streams.into_iter().map(Value::Stream).collect(),
    ))
}

pub(super) fn make_concatenated_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let streams = stream_arguments("make-concatenated-stream", arguments, true)?;
    Ok(Value::concatenated_stream(streams))
}

pub(super) fn make_echo_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-echo-stream", 2)?;
    let input = stream_argument("make-echo-stream", &arguments[0], true)?;
    let output = stream_argument("make-echo-stream", &arguments[1], false)?;
    Ok(Value::echo_stream(input, output))
}

pub(super) fn echo_stream_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "echo-stream-input-stream", 1)?;
    let stream = stream_reference("echo-stream-input-stream", &arguments[0])?;
    let input = stream
        .borrow()
        .echo_input_stream()
        .ok_or_else(|| stream_state_error("echo-stream-input-stream", "an echo stream"))?;
    Ok(Value::Stream(input))
}

pub(super) fn echo_stream_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "echo-stream-output-stream", 1)?;
    let stream = stream_reference("echo-stream-output-stream", &arguments[0])?;
    let output = stream
        .borrow()
        .echo_output_stream()
        .ok_or_else(|| stream_state_error("echo-stream-output-stream", "an echo stream"))?;
    Ok(Value::Stream(output))
}

pub(super) fn pathname_argument(function: &str, value: &Value) -> Result<PathBuf, RuntimeError> {
    match value {
        Value::String(value) => Ok(PathBuf::from(value.as_ref())),
        value => Err(type_error(function, "a string pathname", value)),
    }
}

pub(super) fn open_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("open", "at least 1", arguments.len()));
    }
    if (arguments.len() - 1) % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "open requires keyword/value pairs after the pathname".to_string(),
            span: None,
        });
    }
    let path = pathname_argument("open", &arguments[0])?;
    let mut direction = "INPUT".to_string();
    let mut if_does_not_exist = None;
    let mut if_exists = None;
    let mut element_type = None;
    let mut external_format = None;
    for pair in arguments[1..].chunks_exact(2) {
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
            "ELEMENT-TYPE" => {
                if element_type.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "open received duplicate :element-type".to_string(),
                        span: None,
                    });
                }
                element_type = Some(type_designator_name("open :element-type", &pair[1])?);
            }
            "EXTERNAL-FORMAT" => {
                if external_format.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "open received duplicate :external-format".to_string(),
                        span: None,
                    });
                }
                external_format = Some(type_designator_name("open :external-format", &pair[1])?);
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open does not recognize keyword :{keyword}"),
                    span: None,
                });
            }
        }
    }
    if let Some(element_type) = element_type {
        if element_type != "CHARACTER" {
            return Err(RuntimeError::InvalidForm {
                message: format!("open only supports :element-type CHARACTER, got {element_type}"),
                span: None,
            });
        }
    }
    if let Some(external_format) = external_format {
        if external_format != "DEFAULT" && external_format != "UTF-8" {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "open only supports :external-format DEFAULT or UTF-8, got {external_format}"
                ),
                span: None,
            });
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
                Ok(Value::file_probe_stream())
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

pub(super) fn probe_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn delete_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "delete-file", 1)?;
    let path = pathname_argument("delete-file", &arguments[0])?;
    std::fs::remove_file(&path)
        .map_err(|error| RuntimeError::Io(format!("delete-file {}: {error}", path.display())))?;
    Ok(Value::boolean(true))
}

pub(super) fn rename_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn file_write_date(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn truename(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "truename", 1)?;
    let path = pathname_argument("truename", &arguments[0])?;
    let canonical = std::fs::canonicalize(&path)
        .map_err(|error| RuntimeError::Io(format!("truename {}: {error}", path.display())))?;
    Ok(Value::string(canonical.to_string_lossy().to_string()))
}

pub(super) fn stream_keyword_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Ok(normalize_name(name)),
        value => Err(type_error(function, "a keyword", value)),
    }
}

pub(super) fn open_input_file(
    path: &std::path::Path,
    if_does_not_exist: &str,
) -> Result<Value, RuntimeError> {
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
    Ok(Value::file_input_stream(source))
}

pub(super) fn open_output_file(
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
                return Ok(Value::file_output_stream(path.to_path_buf(), source, true));
            }
            "OVERWRITE" => {
                let source = std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?;
                return Ok(Value::file_output_stream(path.to_path_buf(), source, false));
            }
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "SUPERSEDE" => {}
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
    Ok(Value::file_output_stream(
        path.to_path_buf(),
        String::new(),
        false,
    ))
}

pub(super) fn open_io_file(
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
    Ok(Value::file_io_stream(path.to_path_buf(), source, append))
}

pub(super) fn stream_reference<'a>(
    function: &str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Value::Stream(stream) => Ok(stream),
        value => Err(type_error(function, "a stream", value)),
    }
}

pub(super) fn stream_argument(
    function: &str,
    value: &Value,
    input: bool,
) -> Result<Rc<RefCell<Stream>>, RuntimeError> {
    let stream = stream_reference(function, value)?.clone();
    let valid = if input {
        stream.borrow().is_input()
    } else {
        stream.borrow().is_output()
    };
    if !valid {
        return Err(stream_state_error(
            function,
            if input {
                "an input stream"
            } else {
                "an output stream"
            },
        ));
    }
    Ok(stream)
}

pub(super) fn stream_arguments(
    function: &str,
    arguments: &[Value],
    input: bool,
) -> Result<Vec<Rc<RefCell<Stream>>>, RuntimeError> {
    arguments
        .iter()
        .map(|value| stream_argument(function, value, input))
        .collect()
}

pub(super) fn input_stream_reference<'a>(
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

pub(super) fn stream_state_error(function: &str, expected: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} requires {expected}"),
        span: None,
    }
}

pub(super) fn end_of_file_error(context: &'static str) -> RuntimeError {
    RuntimeError::Read(ReadError::new(
        ReadErrorKind::UnexpectedEnd { context },
        Span::new(0, 0),
    ))
}

pub(super) fn peek_character(
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

pub(super) fn get_output_stream_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-output-stream-string", 1)?;
    let stream = stream_reference("get-output-stream-string", &arguments[0])?;
    let output = stream
        .borrow_mut()
        .take_output()
        .ok_or_else(|| stream_state_error("get-output-stream-string", "an output stream"))?;
    Ok(Value::string(output))
}

pub(super) fn read_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-char", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-char", arguments.first())?;
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
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

pub(super) fn peek_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    let eof_error_p = arguments.get(optional_index).map_or(true, Value::is_truthy);
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

pub(super) fn unread_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn listen(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("listen", "0 or 1", arguments.len()));
    }
    let stream = input_stream_reference("listen", arguments.first())?;
    let stream = stream.borrow();
    if !stream.is_open() {
        return Err(stream_state_error("listen", "an open input stream"));
    }
    if !stream.is_input() {
        return Err(stream_state_error("listen", "an input stream"));
    }
    Ok(Value::boolean(stream.peek_char().is_some()))
}

pub(super) fn clear_input(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("clear-input", "0 or 1", arguments.len()));
    }
    let stream = input_stream_reference("clear-input", arguments.first())?;
    let mut stream = stream.borrow_mut();
    if !stream.is_open() {
        return Err(stream_state_error("clear-input", "an open input stream"));
    }
    if !stream.is_input() {
        return Err(stream_state_error("clear-input", "an input stream"));
    }
    if !stream.clear_input() {
        return Err(stream_state_error("clear-input", "an open input stream"));
    }
    Ok(Value::Nil)
}

pub(super) fn file_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("file-position", "1 or 2", arguments.len()));
    }
    let stream = stream_reference("file-position", &arguments[0])?;
    let mut stream = stream.borrow_mut();
    if !stream.is_open() {
        return Err(stream_state_error("file-position", "an open stream"));
    }
    if arguments.len() == 1 {
        return Ok(stream
            .file_position()
            .map_or(Value::Nil, |position| Value::Integer(position as i64)));
    }
    if stream.file_position().is_none() {
        return Ok(Value::boolean(false));
    }
    let position = file_position_spec("file-position", &stream, &arguments[1])?;
    let length = stream
        .file_length()
        .ok_or_else(|| stream_state_error("file-position", "a file stream"))?;
    if position > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("file-position {position} is beyond file length {length}"),
            span: None,
        });
    }
    Ok(Value::boolean(stream.set_file_position(position)))
}

pub(super) fn file_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-length", 1)?;
    let stream = stream_reference("file-length", &arguments[0])?;
    let stream = stream.borrow();
    if !stream.is_open() {
        return Err(stream_state_error("file-length", "an open file stream"));
    }
    let length = stream
        .file_length()
        .ok_or_else(|| stream_state_error("file-length", "a file stream"))?;
    Ok(Value::Integer(length as i64))
}

pub(super) fn file_position_spec(
    function: &str,
    stream: &Stream,
    value: &Value,
) -> Result<usize, RuntimeError> {
    match value {
        Value::Integer(position) if *position >= 0 => Ok(*position as usize),
        Value::Integer(_) => Err(RuntimeError::InvalidForm {
            message: format!("{function} requires a non-negative file position"),
            span: None,
        }),
        Value::Keyword(_) | Value::KeywordExact(_) => {
            match stream_keyword_name(function, value)?.as_str() {
                "START" => Ok(0),
                "END" => stream
                    .file_length()
                    .ok_or_else(|| stream_state_error(function, "a file stream")),
                _ => Err(RuntimeError::InvalidForm {
                    message: format!("{function} accepts only an integer, :start, or :end"),
                    span: None,
                }),
            }
        }
        value => Err(type_error(function, "an integer or :start/:end", value)),
    }
}

pub(super) fn stream_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stream-element-type", 1)?;
    let stream = stream_reference("stream-element-type", &arguments[0])?;
    let stream = stream.borrow();
    if !stream.is_open() {
        return Err(stream_state_error("stream-element-type", "an open stream"));
    }
    if !stream.is_input() && !stream.is_output() {
        return Err(stream_state_error(
            "stream-element-type",
            "a character stream",
        ));
    }
    Ok(Value::symbol(stream.element_type()))
}

pub(super) fn read_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-line", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-line", arguments.first())?;
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
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

pub(super) fn read_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("read-sequence", "at least 2", arguments.len()));
    }
    let sequence = &arguments[0];
    if !sequence.is_simple_vector() {
        return Err(type_error(
            "read-sequence",
            "a mutable simple vector",
            sequence,
        ));
    }
    let length = sequence_length(sequence)
        .ok_or_else(|| type_error("read-sequence", "a mutable sequence", sequence))?;
    let options = &arguments[2..];
    if options.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "read-sequence keyword arguments must be name/value pairs".to_string(),
            span: None,
        });
    }
    let (start, end) = sequence_bounds("read-sequence", length, options)?;
    let stream = input_stream_reference("read-sequence", Some(&arguments[1]))?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-sequence", "an input stream"));
    }
    let mut position = start;
    while position < end {
        let Some(character) = stream.read_char() else {
            break;
        };
        if !sequence.set_sequence_item(position, Value::Character(character)) {
            return Err(type_error(
                "read-sequence",
                "a mutable character sequence",
                sequence,
            ));
        }
        position += 1;
    }
    Ok(Value::Integer(position as i64))
}

pub(super) fn write_destination(
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

pub(super) fn write_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn string_output_arguments<'a>(
    function: &str,
    arguments: &'a [Value],
) -> Result<(String, Option<&'a Value>), RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least 1", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error(function, "a string", value)),
    };
    let length = string.chars().count();
    let mut start = 0;
    let mut end = length;
    let keyword_arguments = arguments.get(2..).unwrap_or_default();
    if keyword_arguments.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} keyword arguments must be name/value pairs"),
            span: None,
        });
    }
    for pair in keyword_arguments.chunks_exact(2) {
        let keyword = stream_keyword_name(function, &pair[0])?;
        match keyword.as_str() {
            "START" => start = string_bound(function, &pair[1], length)?,
            "END" => {
                end = if matches!(&pair[1], Value::Nil) {
                    length
                } else {
                    string_bound(function, &pair[1], length)?
                };
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not recognize keyword :{keyword}"),
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
    let text = string
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    Ok((text, arguments.get(1)))
}

pub(super) fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (text, destination) = string_output_arguments("write-string", arguments)?;
    write_destination("write-string", destination, &text)?;
    Ok(arguments[0].clone())
}

pub(super) fn terpri(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("terpri", "0 to 1", arguments.len()));
    }
    write_destination("terpri", arguments.first(), "\n")?;
    Ok(Value::Nil)
}

pub(super) fn fresh_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn clear_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("clear-output", "0 or 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => Ok(Value::Nil),
        Some(value) => {
            let stream = stream_reference("clear-output", value)?;
            let mut stream = stream.borrow_mut();
            if !stream.is_open() {
                return Err(stream_state_error("clear-output", "an open output stream"));
            }
            if !stream.is_output() {
                return Ok(Value::Nil);
            }
            if !stream.clear_output() {
                return Err(stream_state_error("clear-output", "an open output stream"));
            }
            Ok(Value::Nil)
        }
    }
}

fn flush_output(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity(function, "0 or 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            std::io::stdout()
                .flush()
                .map_err(|error| RuntimeError::Io(format!("{function}: {error}")))?;
        }
        Some(value) => {
            let stream = stream_reference(function, value)?;
            let mut stream = stream.borrow_mut();
            if !stream.is_open() {
                return Err(stream_state_error(function, "an open output stream"));
            }
            if !stream.is_output() {
                return Err(stream_state_error(function, "an output stream"));
            }
            stream
                .flush()
                .map_err(|error| RuntimeError::Io(format!("{function}: {error}")))?;
        }
    }
    Ok(Value::Nil)
}

pub(super) fn finish_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    flush_output("finish-output", arguments)
}

pub(super) fn force_output(arguments: &[Value]) -> Result<Value, RuntimeError> {
    flush_output("force-output", arguments)
}

pub(super) fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (text, destination) = string_output_arguments("write-line", arguments)?;
    let mut line = String::with_capacity(text.len() + 1);
    line.push_str(&text);
    line.push('\n');
    write_destination("write-line", destination, &line)?;
    Ok(arguments[0].clone())
}

pub(super) fn close_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn open_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "open-stream-p", 1)?;
    let stream = stream_reference("open-stream-p", &arguments[0])?;
    Ok(Value::boolean(stream.borrow().is_open()))
}

pub(super) fn streamp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "streamp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Stream(_))))
}

pub(super) fn input_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "input-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_input(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

pub(super) fn output_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "output-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_output(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

macro_rules! stream_builtins {
    () => {
fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or(arguments[0].type_name()),
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
        Value::Vector { .. } => {
            let values = value.vector_items().expect("vector items");
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

fn stream_input_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "%stream-input-position", 1)?;
    let stream = input_stream_reference("%stream-input-position", arguments.first())?;
    let position = {
        let stream = stream.borrow();
        if !stream.is_input() {
            return Err(stream_state_error(
                "%stream-input-position",
                "an input stream",
            ));
        }
        stream
            .input_position()
            .ok_or_else(|| stream_state_error("%stream-input-position", "an open input stream"))?
    };
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::Integer(position))
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
                Ok(Value::file_input_stream(String::new()))
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
    Ok(Value::file_input_stream(source))
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
    Ok(Value::file_io_stream(path.to_path_buf(), source, append))
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

fn append_output_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__ncl_append_output_to_string", 2)?;
    let Value::Vector {
        elements,
        fill_pointer: Some(fill_pointer),
        element_type,
        adjustable,
        ..
    } = &arguments[0]
    else {
        return Err(type_error(
            "__ncl_append_output_to_string",
            "vector with fill pointer",
            &arguments[0],
        ));
    };

    let mut combined = Vec::with_capacity(*fill_pointer);
    for item in elements.borrow().iter().take(*fill_pointer) {
        let Value::Character(_) = item else {
            return Err(type_error(
                "__ncl_append_output_to_string",
                "characters in vector with fill pointer",
                item,
            ));
        };
        combined.push(item.clone());
    }

    let Value::String(output) = &arguments[1] else {
        return Err(type_error(
            "__ncl_append_output_to_string",
            "string",
            &arguments[1],
        ));
    };
    combined.extend(output.chars().map(Value::Character));
    let new_fill_pointer = combined.len();
    Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
        combined,
        Some(new_fill_pointer),
        element_type.as_ref().clone(),
        *adjustable,
    ))
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


    };
}

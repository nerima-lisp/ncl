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

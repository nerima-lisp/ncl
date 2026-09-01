#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_stream_operation_instruction(
    operation: &str,
    argument_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("stream operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "TERPRI" => crate::builtins::terpri(&arguments),
        "FRESH-LINE" => crate::builtins::fresh_line(&arguments),
        "FORCE-OUTPUT" => crate::builtins::force_output(&arguments),
        "FINISH-OUTPUT" => crate::builtins::finish_output(&arguments),
        "CLEAR-OUTPUT" => crate::builtins::clear_output(&arguments),
        "WRITE-CHAR" => crate::builtins::write_char(&arguments),
        "WRITE-STRING" => crate::builtins::write_string(&arguments),
        "WRITE-LINE" => crate::builtins::write_line(&arguments),
        "WRITE-SEQUENCE" => crate::builtins::write_sequence(&arguments),
        "PRINC" => crate::builtins::princ(&arguments),
        "PRIN1" => crate::builtins::prin1(&arguments),
        "PRINT" => crate::builtins::print_value(&arguments),
        "WRITE" => crate::builtins::write_value(&arguments),
        "GET-OUTPUT-STREAM-STRING" => crate::builtins::get_output_stream_string(&arguments),
        "READ-CHAR" => crate::builtins::read_char(&arguments),
        "READ-CHAR-NO-HANG" => crate::builtins::read_char_no_hang(&arguments),
        "LISTEN" => crate::builtins::listen(&arguments),
        "CLEAR-INPUT" => crate::builtins::clear_input(&arguments),
        "READ-SEQUENCE" => crate::builtins::read_sequence(&arguments),
        "READ-LINE" => crate::builtins::read_line(&arguments),
        "PEEK-CHAR" => crate::builtins::peek_char(&arguments),
        "UNREAD-CHAR" => crate::builtins::unread_char(&arguments),
        "CLOSE" => crate::builtins::close_stream(&arguments),
        "MAKE-STRING-INPUT-STREAM" => crate::builtins::make_string_input_stream(&arguments),
        "MAKE-STRING-OUTPUT-STREAM" => crate::builtins::make_string_output_stream(&arguments),
        "WRITE-TO-STRING" => crate::builtins::write_to_string(&arguments),
        "READ-FROM-STRING" => crate::builtins::read_from_string(&arguments),
        "READ" => crate::builtins::read(&arguments),
        "READ-PRESERVING-WHITESPACE" => crate::builtins::read_preserving_whitespace(&arguments),
        "STREAM-ELEMENT-TYPE" => crate::builtins::stream_element_type(&arguments),
        "STREAM-EXTERNAL-FORMAT" => crate::builtins::stream_external_format(&arguments),
        "FILE-POSITION" => crate::builtins::file_position(&arguments),
        "FILE-LENGTH" => crate::builtins::file_length(&arguments),
        _ => Err(invalid("unknown stream operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_integer_operation_instruction(
    operation: &str,
    argument_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("integer operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "PARSE-INTEGER" => crate::builtins::parse_integer(&arguments),
        _ => Err(invalid("unknown integer operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_file_operation_instruction(
    operation: &str,
    argument_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("file operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "OPEN" => crate::builtins::open_file(&arguments),
        _ => Err(invalid("unknown file operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_file_metadata_operation_instruction(
    operation: &str,
    argument_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "file metadata operation has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "PROBE-FILE" => crate::builtins::probe_file(&arguments),
        "DELETE-FILE" => crate::builtins::delete_file(&arguments),
        "RENAME-FILE" => crate::builtins::rename_file(&arguments),
        "FILE-WRITE-DATE" => crate::builtins::file_write_date(&arguments),
        "TRUENAME" => crate::builtins::truename(&arguments),
        _ => Err(invalid("unknown file metadata operation", span)),
    }?;
    stack.push(result);
    Ok(())
}


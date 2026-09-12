use std::fs;
use std::time::SystemTime;

use crate::RuntimeError;
use crate::builtins::*;

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system clock before unix epoch: {error}"))
        .as_nanos()
}

#[test]
fn stream_predicates_reject_bad_arity() {
    assert!(streamp(&[]).is_err());
    assert!(streamp(&[Value::Nil, Value::Nil]).is_err());
    assert!(input_stream_p(&[]).is_err());
    assert!(input_stream_p(&[Value::Nil, Value::Nil]).is_err());
    assert!(output_stream_p(&[]).is_err());
    assert!(output_stream_p(&[Value::Nil, Value::Nil]).is_err());
    assert!(open_stream_p(&[]).is_err());
    assert!(open_stream_p(&[Value::Nil, Value::Nil]).is_err());
    assert!(stream_element_type(&[]).is_err());
    assert!(stream_element_type(&[Value::Nil, Value::Nil]).is_err());
    assert!(stream_external_format(&[]).is_err());
    assert!(stream_external_format(&[Value::Nil, Value::Nil]).is_err());
    assert!(file_length(&[]).is_err());
    assert!(file_length(&[Value::Nil, Value::Nil]).is_err());
}

#[test]
fn stream_predicates_report_non_stream_values_as_false() -> Result<(), RuntimeError> {
    assert!(matches!(streamp(&[Value::Nil])?, Value::Nil));
    assert!(matches!(input_stream_p(&[Value::Integer(1)])?, Value::Nil));
    assert!(matches!(output_stream_p(&[Value::Integer(1)])?, Value::Nil));
    Ok(())
}

#[test]
fn stream_metadata_reports_kind_open_state_and_file_length() -> Result<(), RuntimeError> {
    let string_input = make_string_input_stream(&[Value::string("abc")])?;
    assert!(open_stream_p(std::slice::from_ref(&string_input))?.is_truthy());
    assert_eq!(
        stream_element_type(std::slice::from_ref(&string_input))?.to_string(),
        "CHARACTER"
    );
    assert!(matches!(
        stream_external_format(std::slice::from_ref(&string_input))?,
        Value::Nil
    ));
    close_stream(std::slice::from_ref(&string_input))?;
    assert!(!open_stream_p(std::slice::from_ref(&string_input))?.is_truthy());

    let binary_input = Value::binary_input_stream(vec![1, 2, 3]);
    assert_eq!(
        stream_element_type(std::slice::from_ref(&binary_input))?.to_string(),
        "(UNSIGNED-BYTE 8)"
    );
    assert_eq!(
        stream_external_format(std::slice::from_ref(&binary_input))?.to_string(),
        ":DEFAULT"
    );

    let file_input = Value::file_input_stream("abc");
    assert_eq!(
        file_length(std::slice::from_ref(&file_input))?.to_string(),
        "3"
    );
    assert!(file_length(&[Value::string("abc")]).is_err());
    Ok(())
}

#[test]
fn output_controls_flush_and_clear_pending_output() -> Result<(), RuntimeError> {
    assert!(force_output(&[Value::Nil, Value::Nil]).is_err());
    assert!(finish_output(&[Value::Nil, Value::Nil]).is_err());
    assert!(clear_output(&[Value::Nil, Value::Nil]).is_err());

    let string_output = make_string_output_stream(&[])?;
    write_string(&[Value::string("discard"), string_output.clone()])?;
    clear_output(std::slice::from_ref(&string_output))?;
    assert!(matches!(
        get_output_stream_string(std::slice::from_ref(&string_output))?,
        Value::String(text) if text.as_ref().is_empty()
    ));
    write_string(&[Value::string("kept"), string_output.clone()])?;
    force_output(std::slice::from_ref(&string_output))?;
    write_string(&[Value::string("discard"), string_output.clone()])?;
    clear_output(std::slice::from_ref(&string_output))?;
    finish_output(std::slice::from_ref(&string_output))?;
    assert!(matches!(
        get_output_stream_string(&[string_output])?,
        Value::String(text) if text.as_ref() == "kept"
    ));

    let directory = std::env::temp_dir().join(format!("ncl-output-controls-{}", nonce()));
    fs::create_dir(&directory).unwrap_or_else(|error| panic!("create scratch dir: {error}"));
    let path = directory.join("out.txt");
    let output = open_file(&[
        Value::string(path.to_string_lossy().to_string()),
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("if-exists"),
        Value::keyword("supersede"),
    ])?;
    write_string(&[Value::string("new"), output.clone()])?;
    force_output(std::slice::from_ref(&output))?;
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    write_string(&[Value::string("!"), output.clone()])?;
    clear_output(std::slice::from_ref(&output))?;
    finish_output(std::slice::from_ref(&output))?;
    close_stream(std::slice::from_ref(&output))?;
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    fs::remove_dir_all(directory).unwrap_or_else(|error| panic!("remove scratch dir: {error}"));
    Ok(())
}

#[test]
fn close_stream_rejects_bad_arity_and_argument_types() {
    assert!(close_stream(&[]).is_err());
    assert!(close_stream(&[Value::Nil, Value::Nil]).is_err());
    assert!(close_stream(&[Value::Integer(1)]).is_err());
}

#[test]
fn close_stream_rejects_a_malformed_abort_keyword() -> Result<(), RuntimeError> {
    let stream = make_string_output_stream(&[])?;
    assert!(close_stream(&[stream.clone(), Value::Integer(1), Value::Nil]).is_err());
    assert!(close_stream(&[stream, Value::keyword("unknown"), Value::Nil]).is_err());
    Ok(())
}

#[test]
fn close_stream_reports_io_errors_from_a_failed_flush() -> Result<(), RuntimeError> {
    let directory = std::env::temp_dir().join(format!("ncl-close-stream-error-{}", nonce()));
    fs::create_dir(&directory).unwrap_or_else(|error| panic!("create scratch dir: {error}"));
    let path = directory.join("out.txt");
    let output = open_file(&[
        Value::string(path.to_string_lossy().to_string()),
        Value::keyword("direction"),
        Value::keyword("output"),
    ])?;
    fs::remove_dir_all(&directory).unwrap_or_else(|error| panic!("remove scratch dir: {error}"));
    let result = close_stream(&[output]);
    assert!(matches!(result, Err(RuntimeError::Io { .. })));
    Ok(())
}

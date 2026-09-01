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
    assert!(stream_external_format(&[Value::Nil, Value::Nil]).is_err());
}

#[test]
fn stream_predicates_report_non_stream_values_as_false() -> Result<(), RuntimeError> {
    assert!(matches!(streamp(&[Value::Nil])?, Value::Nil));
    assert!(matches!(input_stream_p(&[Value::Integer(1)])?, Value::Nil));
    assert!(matches!(output_stream_p(&[Value::Integer(1)])?, Value::Nil));
    assert!(matches!(open_stream_p(&[Value::Integer(1)])?, Value::Nil));
    Ok(())
}

#[test]
fn open_stream_p_tracks_close_state() -> Result<(), RuntimeError> {
    let stream = make_string_output_stream(&[])?;
    assert!(open_stream_p(std::slice::from_ref(&stream))?.is_truthy());
    close_stream(std::slice::from_ref(&stream))?;
    assert!(!open_stream_p(&[stream])?.is_truthy());
    Ok(())
}

#[test]
fn stream_metadata_reports_character_default_and_rejects_closed_streams() -> Result<(), RuntimeError> {
    let stream = make_string_output_stream(&[])?;
    assert_eq!(stream_element_type(std::slice::from_ref(&stream))?.to_string(), "CHARACTER");
    assert_eq!(stream_external_format(std::slice::from_ref(&stream))?.to_string(), ":DEFAULT");
    close_stream(std::slice::from_ref(&stream))?;
    assert!(stream_element_type(std::slice::from_ref(&stream)).is_err());
    assert!(stream_external_format(&[stream]).is_err());
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

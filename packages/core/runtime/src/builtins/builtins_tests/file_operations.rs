use crate::RuntimeError;
use crate::builtins::builtin_printer::parse_print_options;
use crate::builtins::*;

fn nanosecond_suffix() -> Result<u128, RuntimeError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|error| RuntimeError::InvalidForm {
            message: error.to_string(),
            span: None,
        })
}

#[test]
fn print_and_file_builtins_reject_invalid_options() {
    assert!(print_value(&[]).is_err());
    assert!(princ(&[]).is_err());
    assert!(prin1(&[]).is_err());
    assert!(write_value(&[]).is_err());
    assert!(write_to_string(&[]).is_err());
    assert!(
        write_to_string(&[Value::string("text"), Value::keyword("stream"), Value::Nil]).is_err()
    );
    assert!(parse_print_options("test", &[Value::keyword("escape")], false).is_err());
    assert!(parse_print_options("test", &[Value::keyword("unknown"), Value::Nil], false).is_err());
    assert!(open_file(&[]).is_err());
    assert!(open_file(&[Value::Integer(1)]).is_err());
    assert!(
        open_file(&[
            Value::string("missing"),
            Value::keyword("unknown"),
            Value::Nil
        ])
        .is_err()
    );
    assert!(probe_file(&[Value::Integer(1)]).is_err());
    assert!(delete_file(&[Value::Integer(1)]).is_err());
    assert!(rename_file(&[]).is_err());
    assert!(rename_file(&[Value::Integer(1), Value::Integer(2)]).is_err());
    assert!(file_write_date(&[Value::Integer(1)]).is_err());
    assert!(truename(&[Value::Integer(1)]).is_err());
    assert!(make_string_output_stream(&[Value::Integer(1)]).is_err());
    assert!(get_output_stream_string(&[Value::Integer(1)]).is_err());
}

#[test]
fn file_operations_cover_lifecycle_and_open_directions() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let root = std::env::temp_dir().join(format!("ncl-runtime-{suffix}"));
    let path = Value::string(root.to_string_lossy().to_string());
    let output = open_file(&[
        path.clone(),
        Value::keyword("direction"),
        Value::keyword("output"),
    ])?;
    write_line(&[Value::string("line"), output.clone()])?;
    close_stream(&[output])?;
    assert!(matches!(
        probe_file(std::slice::from_ref(&path))?,
        Value::String(_)
    ));
    assert!(matches!(
        open_file(&[
            path.clone(),
            Value::keyword("direction"),
            Value::keyword("probe"),
        ])?,
        Value::Stream(_)
    ));
    let input = open_file(std::slice::from_ref(&path))?;
    assert!(matches!(
        read_line(std::slice::from_ref(&input))?.primary_value(),
        Value::String(text) if text.as_ref() == "line"
    ));
    close_stream(&[input])?;
    let renamed = Value::string(format!("{}-renamed", root.display()));
    assert!(matches!(
        rename_file(&[path.clone(), renamed.clone()])?,
        Value::Values(_)
    ));
    assert!(
        file_write_date(std::slice::from_ref(&renamed))?
            .to_string()
            .parse::<i64>()
            .is_ok()
    );
    assert!(
        truename(std::slice::from_ref(&renamed))?
            .to_string()
            .contains("ncl-runtime-")
    );
    assert!(matches!(delete_file(&[renamed])?, Value::Boolean(true)));
    assert!(matches!(probe_file(&[path])?, Value::Nil));
    Ok(())
}

#[test]
fn byte_file_streams_read_write_and_append_raw_bytes() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let root = std::env::temp_dir().join(format!("ncl-byte-stream-{suffix}"));
    std::fs::write(&root, [1_u8, 2_u8]).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    let path = Value::string(root.to_string_lossy().to_string());
    let byte_type = Value::list(vec![Value::symbol("UNSIGNED-BYTE"), Value::Integer(8)]);

    let input = open_file(&[
        path.clone(),
        Value::keyword("element-type"),
        byte_type.clone(),
    ])?;
    assert_eq!(
        stream_element_type(std::slice::from_ref(&input))?.to_string(),
        "(UNSIGNED-BYTE 8)"
    );
    assert!(matches!(
        read_byte(std::slice::from_ref(&input))?,
        Value::Integer(1)
    ));
    assert!(matches!(
        read_byte(std::slice::from_ref(&input))?,
        Value::Integer(2)
    ));
    assert!(matches!(
        read_byte(&[input.clone(), Value::Integer(9), Value::Nil])?,
        Value::Integer(9)
    ));
    close_stream(&[input])?;

    let output = open_file(&[
        path.clone(),
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("if-exists"),
        Value::keyword("append"),
        Value::keyword("element-type"),
        byte_type,
    ])?;
    assert!(matches!(
        write_byte(&[Value::Integer(3), output.clone()])?,
        Value::Integer(3)
    ));
    close_stream(&[output])?;
    let bytes = std::fs::read(&root).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    assert_eq!(bytes, vec![1, 2, 3]);
    Ok(())
}

#[test]
fn byte_streams_support_sequence_io() -> Result<(), RuntimeError> {
    let input = Value::file_byte_input_stream(vec![4, 5, 6]);
    let destination = Value::vector(vec![Value::Nil, Value::Nil, Value::Nil, Value::Nil]);
    assert!(matches!(
        read_sequence(&[
            destination.clone(),
            input,
            Value::keyword("start"),
            Value::Integer(1),
            Value::keyword("end"),
            Value::Integer(4),
        ])?,
        Value::Integer(4)
    ));
    assert_eq!(
        destination
            .vector_items()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>(),
        vec!["NIL", "4", "5", "6"]
    );

    let suffix = nanosecond_suffix()?;
    let path = std::env::temp_dir().join(format!("ncl-byte-sequence-{suffix}"));
    let output = Value::file_byte_output_stream(path.clone(), Vec::new());
    write_sequence(&[
        Value::vector(vec![
            Value::Integer(7),
            Value::Integer(8),
            Value::Integer(9),
        ]),
        output.clone(),
        Value::keyword("start"),
        Value::Integer(1),
        Value::keyword("end"),
        Value::Integer(3),
    ])?;
    close_stream(&[output])?;
    assert_eq!(std::fs::read(&path).unwrap(), vec![8, 9]);
    std::fs::remove_file(path).unwrap();
    Ok(())
}

#[test]
fn byte_io_streams_share_a_file_cursor_for_read_and_write() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let root = std::env::temp_dir().join(format!("ncl-byte-io-{suffix}"));
    std::fs::write(&root, [1_u8, 2_u8, 3_u8]).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    let path = Value::string(root.to_string_lossy().to_string());
    let byte_type = Value::list(vec![Value::symbol("UNSIGNED-BYTE"), Value::Integer(8)]);
    let stream = open_file(&[
        path,
        Value::keyword("direction"),
        Value::keyword("io"),
        Value::keyword("element-type"),
        byte_type,
    ])?;
    assert!(matches!(
        read_byte(std::slice::from_ref(&stream))?,
        Value::Integer(1)
    ));
    assert!(matches!(
        write_byte(&[Value::Integer(9), stream.clone()])?,
        Value::Integer(9)
    ));
    close_stream(&[stream])?;
    assert_eq!(std::fs::read(&root).unwrap(), vec![1, 9, 3]);
    std::fs::remove_file(root).unwrap();
    Ok(())
}

#[test]
fn byte_output_stream_file_position_repositions_writes() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let path = std::env::temp_dir().join(format!("ncl-byte-output-position-{suffix}"));
    let stream = Value::file_byte_output_stream(path.clone(), vec![1, 2, 3]);

    assert_eq!(
        file_position(std::slice::from_ref(&stream))?.to_string(),
        "3"
    );
    assert_eq!(
        file_position(&[stream.clone(), Value::Integer(1)])?.to_string(),
        "1"
    );
    write_byte(&[Value::Integer(9), stream.clone()])?;
    close_stream(&[stream])?;

    assert_eq!(std::fs::read(&path).unwrap(), vec![1, 9, 3]);
    std::fs::remove_file(path).unwrap();
    Ok(())
}

#[test]
fn open_keyword_options_cover_defaults_and_validation() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let missing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-missing"));
    let existing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-existing"));
    std::fs::write(&existing_path, "content").map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    let missing = Value::string(missing_path.to_string_lossy().to_string());
    let existing = Value::string(existing_path.to_string_lossy().to_string());

    assert!(open_file(std::slice::from_ref(&missing)).is_err());
    assert!(matches!(
        open_file(&[
            missing.clone(),
            Value::keyword("if-does-not-exist"),
            Value::keyword("nil"),
        ])?,
        Value::Nil
    ));
    let output = open_file(&[
        missing,
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("element-type"),
        Value::keyword("character"),
        Value::keyword("external-format"),
        Value::keyword("utf-8"),
    ])?;
    close_stream(&[output])?;
    let io = open_file(&[
        existing.clone(),
        Value::keyword("direction"),
        Value::keyword("io"),
        Value::keyword("if-exists"),
        Value::keyword("overwrite"),
    ])?;
    close_stream(&[io])?;

    assert!(
        open_file(&[
            existing.clone(),
            Value::keyword("element-type"),
            Value::keyword("unsigned-byte"),
        ])
        .is_err()
    );

    assert!(open_file(&[existing.clone(), Value::keyword("unknown"), Value::Nil]).is_err());
    assert!(open_file(&[existing.clone(), Value::keyword("direction")]).is_err());
    assert!(
        open_file(&[
            existing,
            Value::keyword("direction"),
            Value::keyword("unknown"),
        ])
        .is_err()
    );

    let _ = std::fs::remove_file(missing_path);
    let _ = std::fs::remove_file(existing_path);
    Ok(())
}

#[test]
fn open_supersede_replaces_existing_file_contents() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let root = std::env::temp_dir().join(format!("ncl-open-supersede-{suffix}"));
    std::fs::write(&root, "old content").map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    let path = Value::string(root.to_string_lossy().to_string());
    let stream = open_file(&[
        path,
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("if-exists"),
        Value::keyword("supersede"),
    ])?;
    write_line(&[Value::string("new"), stream.clone()])?;
    close_stream(&[stream])?;
    assert_eq!(std::fs::read_to_string(&root).unwrap(), "new\n");
    std::fs::remove_file(root).unwrap();
    Ok(())
}

#[test]
fn open_overwrite_writes_from_the_start_of_existing_file() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let root = std::env::temp_dir().join(format!("ncl-open-overwrite-{suffix}"));
    std::fs::write(&root, "old content").map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: error.to_string(),
    })?;
    let stream = open_file(&[
        Value::string(root.to_string_lossy().to_string()),
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("if-exists"),
        Value::keyword("overwrite"),
    ])?;
    write_string(&[Value::string("new"), stream.clone()])?;
    close_stream(&[stream])?;
    assert_eq!(std::fs::read_to_string(&root).unwrap(), "new content");
    std::fs::remove_file(root).unwrap();
    Ok(())
}

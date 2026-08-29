use crate::builtins::*;
use crate::RuntimeError;

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
fn open_keyword_options_cover_defaults_and_validation() -> Result<(), RuntimeError> {
    let suffix = nanosecond_suffix()?;
    let missing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-missing"));
    let existing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-existing"));
    std::fs::write(&existing_path, "content")
        .map_err(|error| RuntimeError::Io(error.to_string()))?;
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

use super::{exact, pathname_argument};
use crate::{RuntimeError, Value};

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

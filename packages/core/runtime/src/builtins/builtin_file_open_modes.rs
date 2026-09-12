use std::path::Path;

use crate::{RuntimeError, Value};

fn numbered_path(path: &Path, suffix: &str) -> std::path::PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut index = 0;
    loop {
        let candidate = parent.join(format!("{name}.{suffix}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn rename_existing(path: &Path) -> Result<std::path::PathBuf, RuntimeError> {
    let backup = numbered_path(path, "ncl-rename");
    std::fs::rename(path, &backup).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: format!("rename {} to {}: {error}", path.display(), backup.display()),
    })?;
    Ok(backup)
}

fn set_cleanup(stream: Value, cleanup: Option<std::path::PathBuf>) -> Value {
    if let Some(path) = cleanup {
        let _ = stream.set_stream_delete_on_close(path);
    }
    stream
}

pub(crate) fn open_input_file(path: &Path, if_does_not_exist: &str) -> Result<Value, RuntimeError> {
    if !path.exists() {
        match if_does_not_exist {
            "NIL" => return Ok(Value::Nil),
            "CREATE" => {
                std::fs::write(path, []).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
            }
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: format!("open {}: {error}", path.display()),
    })?;
    Ok(Value::file_input_stream(&source))
}

pub(crate) fn open_output_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut target = path.to_path_buf();
    let mut cleanup = None;
    if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::AlreadyExists,
                    message: format!("open {}: file already exists", path.display()),
                });
            }
            "APPEND" => {
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                return Ok(Value::file_output_stream(path.to_path_buf(), source));
            }
            "OVERWRITE" => {
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                return Ok(Value::file_output_stream_at(path.to_path_buf(), source, 0));
            }
            "NEW-VERSION" => target = numbered_path(path, "ncl-version"),
            "RENAME" => {
                let _ = rename_existing(path)?;
            }
            "RENAME-AND-DELETE" => {
                cleanup = Some(rename_existing(path)?);
            }
            "SUPERSEDE" => {}
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
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    Ok(set_cleanup(
        Value::file_output_stream(target, String::new()),
        cleanup,
    ))
}

pub(crate) fn open_io_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut append = false;
    let mut target = path.to_path_buf();
    let mut cleanup = None;
    let source = if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::AlreadyExists,
                    message: format!("open {}: file already exists", path.display()),
                });
            }
            "APPEND" => {
                append = true;
                std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?
            }
            "OVERWRITE" => std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                kind: error.kind(),
                message: format!("open {}: {error}", path.display()),
            })?,
            "NEW-VERSION" => {
                target = numbered_path(path, "ncl-version");
                String::new()
            }
            "RENAME" => {
                let _ = rename_existing(path)?;
                String::new()
            }
            "RENAME-AND-DELETE" => {
                cleanup = Some(rename_existing(path)?);
                String::new()
            }
            "SUPERSEDE" => String::new(),
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
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    Ok(set_cleanup(
        Value::file_io_stream(target, &source, append),
        cleanup,
    ))
}

pub(crate) fn open_binary_input_file(
    path: &Path,
    if_does_not_exist: &str,
) -> Result<Value, RuntimeError> {
    if !path.exists() {
        match if_does_not_exist {
            "NIL" => return Ok(Value::Nil),
            "CREATE" => {
                std::fs::write(path, []).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
            }
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: format!("open {}: {error}", path.display()),
    })?;
    Ok(Value::binary_input_stream(bytes))
}

pub(crate) fn open_binary_output_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut target = path.to_path_buf();
    let mut cleanup = None;
    if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::AlreadyExists,
                    message: format!("open {}: file already exists", path.display()),
                });
            }
            "APPEND" => {
                let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                let position = bytes.len();
                return Ok(Value::binary_output_stream(
                    path.to_path_buf(),
                    bytes,
                    position,
                ));
            }
            "OVERWRITE" => {
                let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                return Ok(Value::binary_output_stream(path.to_path_buf(), bytes, 0));
            }
            "NEW-VERSION" => target = numbered_path(path, "ncl-version"),
            "RENAME" => {
                let _ = rename_existing(path)?;
            }
            "RENAME-AND-DELETE" => {
                cleanup = Some(rename_existing(path)?);
            }
            "SUPERSEDE" => {}
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
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    Ok(set_cleanup(
        Value::binary_output_stream(target, Vec::new(), 0),
        cleanup,
    ))
}

pub(crate) fn open_binary_io_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut append = false;
    let mut target = path.to_path_buf();
    let mut cleanup = None;
    let bytes = if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::AlreadyExists,
                    message: format!("open {}: file already exists", path.display()),
                });
            }
            "APPEND" => {
                append = true;
                std::fs::read(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?
            }
            "OVERWRITE" => std::fs::read(path).map_err(|error| RuntimeError::Io {
                kind: error.kind(),
                message: format!("open {}: {error}", path.display()),
            })?,
            "NEW-VERSION" => {
                target = numbered_path(path, "ncl-version");
                Vec::new()
            }
            "RENAME" => {
                let _ = rename_existing(path)?;
                Vec::new()
            }
            "RENAME-AND-DELETE" => {
                cleanup = Some(rename_existing(path)?);
                Vec::new()
            }
            "SUPERSEDE" => Vec::new(),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open received unknown :if-exists value :{if_exists}"),
                    span: None,
                });
            }
        }
    } else {
        match if_does_not_exist {
            "CREATE" => Vec::new(),
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::NotFound,
                    message: format!("open {}: file does not exist", path.display()),
                });
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
    Ok(set_cleanup(
        Value::binary_io_stream(target, bytes, append),
        cleanup,
    ))
}

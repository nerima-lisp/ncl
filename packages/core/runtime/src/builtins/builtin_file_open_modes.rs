use std::path::Path;

use crate::{RuntimeError, Value};

pub(super) fn open_input_file(
    path: &Path,
    if_does_not_exist: &str,
    byte: bool,
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
    if byte {
        return std::fs::read(path)
            .map(Value::file_byte_input_stream)
            .map_err(|error| RuntimeError::Io {
                kind: error.kind(),
                message: format!("open {}: {error}", path.display()),
            });
    }
    let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: format!("open {}: {error}", path.display()),
    })?;
    Ok(Value::file_input_stream(&source))
}

pub(super) fn open_output_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
    byte: bool,
) -> Result<Value, RuntimeError> {
    let output_path = if path.exists() && if_exists == "NEW-VERSION" {
        unique_version_path(path)?
    } else {
        path.to_path_buf()
    };
    let delete_on_close = if path.exists() && matches!(if_exists, "RENAME" | "RENAME-AND-DELETE") {
        let backup = unique_rename_path(path)?;
        std::fs::rename(path, &backup).map_err(|error| RuntimeError::Io {
            kind: error.kind(),
            message: format!("rename {}: {error}", path.display()),
        })?;
        (
            if if_exists == "RENAME-AND-DELETE" {
                Some(backup)
            } else {
                None
            },
            true,
        )
    } else {
        (None, false)
    };
    if path.exists() && !delete_on_close.1 {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io {
                    kind: std::io::ErrorKind::AlreadyExists,
                    message: format!("open {}: file already exists", path.display()),
                });
            }
            "APPEND" => {
                if byte {
                    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
                        kind: error.kind(),
                        message: format!("open {}: {error}", path.display()),
                    })?;
                    let stream = Value::file_byte_output_stream(path.to_path_buf(), bytes);
                    if let Some(backup) = delete_on_close.0 {
                        stream.delete_stream_file_on_close(backup);
                    }
                    return Ok(stream);
                }
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                let stream = Value::file_output_stream(path.to_path_buf(), source);
                if let Some(backup) = delete_on_close.0 {
                    stream.delete_stream_file_on_close(backup);
                }
                return Ok(stream);
            }
            "OVERWRITE" => {
                if byte {
                    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
                        kind: error.kind(),
                        message: format!("open {}: {error}", path.display()),
                    })?;
                    return Ok(Value::file_byte_output_stream(path.to_path_buf(), bytes));
                }
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                return Ok(Value::file_output_stream_at(path.to_path_buf(), source, 0));
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
    if byte {
        let stream = Value::file_byte_output_stream(output_path, Vec::new());
        if let Some(backup) = delete_on_close.0 {
            stream.delete_stream_file_on_close(backup);
        }
        Ok(stream)
    } else {
        let stream = Value::file_output_stream(output_path, String::new());
        if let Some(backup) = delete_on_close.0 {
            stream.delete_stream_file_on_close(backup);
        }
        Ok(stream)
    }
}

fn unique_version_path(path: &Path) -> Result<std::path::PathBuf, RuntimeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or_else(|| RuntimeError::InvalidForm {
        message: format!("open cannot version path {}", path.display()),
        span: None,
    })?;
    for index in 0..1000 {
        let candidate = parent.join(format!("{}.ncl-version-{index}", name.to_string_lossy()));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(RuntimeError::Io {
        kind: std::io::ErrorKind::AlreadyExists,
        message: format!(
            "could not find a unique version target for {}",
            path.display()
        ),
    })
}

fn unique_rename_path(path: &Path) -> Result<std::path::PathBuf, RuntimeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or_else(|| RuntimeError::InvalidForm {
        message: format!("open cannot rename path {}", path.display()),
        span: None,
    })?;
    for index in 0..1000 {
        let candidate = parent.join(format!("{}.ncl-rename-{index}", name.to_string_lossy()));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(RuntimeError::Io {
        kind: std::io::ErrorKind::AlreadyExists,
        message: format!(
            "could not find a unique rename target for {}",
            path.display()
        ),
    })
}

pub(super) fn open_io_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
    byte: bool,
) -> Result<Value, RuntimeError> {
    let output_path = if path.exists() && if_exists == "NEW-VERSION" {
        unique_version_path(path)?
    } else {
        path.to_path_buf()
    };
    if byte {
        let bytes = if path.exists() {
            match if_exists {
                "NIL" => return Ok(Value::Nil),
                "ERROR" => {
                    return Err(RuntimeError::Io {
                        kind: std::io::ErrorKind::AlreadyExists,
                        message: format!("open {}: file already exists", path.display()),
                    });
                }
                "APPEND" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" => std::fs::read(path)
                    .map_err(|error| RuntimeError::Io {
                        kind: error.kind(),
                        message: format!("open {}: {error}", path.display()),
                    })?,
                "NEW-VERSION" => Vec::new(),
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
        let stream = Value::file_byte_io_stream(output_path, bytes, if_exists == "APPEND");
        if let Some(backup) = rename_existing_file(path, if_exists)? {
            if if_exists == "RENAME-AND-DELETE" {
                stream.delete_stream_file_on_close(backup);
            }
        }
        return Ok(stream);
    }
    let mut append = false;
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
            "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" => {
                std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?
            }
            "NEW-VERSION" => String::new(),
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
    let stream = Value::file_io_stream(output_path, &source, append);
    if let Some(backup) = rename_existing_file(path, if_exists)? {
        if if_exists == "RENAME-AND-DELETE" {
            stream.delete_stream_file_on_close(backup);
        }
    }
    Ok(stream)
}

fn rename_existing_file(
    path: &Path,
    if_exists: &str,
) -> Result<Option<std::path::PathBuf>, RuntimeError> {
    if !path.exists() || !matches!(if_exists, "RENAME" | "RENAME-AND-DELETE") {
        return Ok(None);
    }
    let backup = unique_rename_path(path)?;
    std::fs::rename(path, &backup).map_err(|error| RuntimeError::Io {
        kind: error.kind(),
        message: format!("rename {}: {error}", path.display()),
    })?;
    Ok(Some(backup))
}

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
    let renamed = if path.exists() && if_exists == "RENAME" {
        let backup = unique_rename_path(path)?;
        std::fs::rename(path, &backup).map_err(|error| RuntimeError::Io {
            kind: error.kind(),
            message: format!("rename {}: {error}", path.display()),
        })?;
        true
    } else {
        false
    };
    if path.exists() && !renamed {
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
                    return Ok(Value::file_byte_output_stream(path.to_path_buf(), bytes));
                }
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?;
                return Ok(Value::file_output_stream(path.to_path_buf(), source));
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
        Ok(Value::file_byte_output_stream(
            path.to_path_buf(),
            Vec::new(),
        ))
    } else {
        Ok(Value::file_output_stream(path.to_path_buf(), String::new()))
    }
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
                "APPEND" | "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" => {
                    std::fs::read(path).map_err(|error| RuntimeError::Io {
                        kind: error.kind(),
                        message: format!("open {}: {error}", path.display()),
                    })?
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
        return Ok(Value::file_byte_io_stream(
            path.to_path_buf(),
            bytes,
            if_exists == "APPEND",
        ));
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
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" => {
                std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?
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
    Ok(Value::file_io_stream(path.to_path_buf(), &source, append))
}

use std::path::Path;

use crate::{RuntimeError, Value};

pub(super) fn open_input_file(path: &Path, if_does_not_exist: &str) -> Result<Value, RuntimeError> {
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

pub(super) fn open_output_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
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
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {}
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
    Ok(Value::file_output_stream(path.to_path_buf(), String::new()))
}

pub(super) fn open_io_file(
    path: &Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
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
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {
                std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(),
                    message: format!("open {}: {error}", path.display()),
                })?
            }
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

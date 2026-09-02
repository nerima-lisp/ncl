use std::path::Path;

use crate::{RuntimeError, Value};

pub(super) fn open_input_file(path: &Path, if_does_not_exist: &str, byte: bool) -> Result<Value, RuntimeError> {
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
        return std::fs::read(path).map(Value::file_byte_input_stream).map_err(|error| RuntimeError::Io {
            kind: error.kind(), message: format!("open {}: {error}", path.display()),
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
    if_exists: &str, byte: bool,
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
                if byte {
                    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io { kind: error.kind(), message: format!("open {}: {error}", path.display()) })?;
                    return Ok(Value::file_byte_output_stream(path.to_path_buf(), bytes));
                }
                let source = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
                    kind: error.kind(), message: format!("open {}: {error}", path.display()),
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
    if byte { Ok(Value::file_byte_output_stream(path.to_path_buf(), Vec::new())) } else { Ok(Value::file_output_stream(path.to_path_buf(), String::new())) }
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
                "ERROR" => return Err(RuntimeError::Io { kind: std::io::ErrorKind::AlreadyExists, message: format!("open {}: file already exists", path.display()) }),
                "APPEND" | "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => std::fs::read(path).map_err(|error| RuntimeError::Io { kind: error.kind(), message: format!("open {}: {error}", path.display()) })?,
                _ => return Err(RuntimeError::InvalidForm { message: format!("open received unknown :if-exists value :{if_exists}"), span: None }),
            }
        } else {
            match if_does_not_exist {
                "CREATE" => Vec::new(),
                "NIL" => return Ok(Value::Nil),
                "ERROR" => return Err(RuntimeError::Io { kind: std::io::ErrorKind::NotFound, message: format!("open {}: file does not exist", path.display()) }),
                _ => return Err(RuntimeError::InvalidForm { message: format!("open received unknown :if-does-not-exist value :{if_does_not_exist}"), span: None }),
            }
        };
        return Ok(Value::file_byte_io_stream(path.to_path_buf(), bytes, if_exists == "APPEND"));
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

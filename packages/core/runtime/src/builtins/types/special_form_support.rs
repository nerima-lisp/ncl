#![allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

pub(crate) fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

pub fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "the", 2)?;
    let type_description = arguments[1].to_string();
    if type_matches_designator("the", &arguments[0], &arguments[1])? {
        Ok(arguments[0].clone())
    } else {
        Err(RuntimeError::Type {
            expected: format!("the requires value of type {type_description}"),
            actual: arguments[0].type_name().to_string(),
            span: None,
        })
    }
}

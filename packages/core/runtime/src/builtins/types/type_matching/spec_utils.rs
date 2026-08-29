use crate::builtins::builtin_helpers::type_error;
use crate::builtins::types::type_matching::dispatch::type_matches_designator;
use crate::{RuntimeError, Value};

pub(in crate::builtins::types) fn require_type_spec_arity(
    function: &str,
    operator: &str,
    arguments: &[Value],
    minimum: usize,
    maximum: usize,
) -> Result<(), RuntimeError> {
    if (minimum..=maximum).contains(&arguments.len()) {
        Ok(())
    } else {
        Err(invalid_type_spec(
            function,
            format!("{operator} type specifier expects between {minimum} and {maximum} arguments"),
        ))
    }
}

pub(in crate::builtins::types) fn invalid_type_spec(
    function: &str,
    message: impl Into<String>,
) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function}: {}", message.into()),
        span: None,
    }
}

pub(in crate::builtins::types) fn is_type_wildcard(value: &Value) -> bool {
    value
        .symbol_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("*"))
}

pub(in crate::builtins::types) fn type_spec_size(
    function: &str,
    value: &Value,
) -> Result<Option<usize>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    let Value::Integer(size) = value else {
        return Err(type_error(function, "non-negative integer or *", value));
    };
    usize::try_from(*size)
        .map(Some)
        .map_err(|_| invalid_type_spec(function, "sequence or array size must be non-negative"))
}

pub(in crate::builtins::types::type_matching) fn type_matches_element_spec(
    function: &str,
    value: &Value,
    type_designator: &Value,
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(type_designator) {
        Ok(true)
    } else {
        type_matches_designator(function, value, type_designator)
    }
}

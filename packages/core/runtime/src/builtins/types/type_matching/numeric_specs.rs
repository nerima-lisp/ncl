use crate::builtins::builtin_helpers::type_error;
use crate::builtins::types::type_matching::spec_utils::{
    invalid_type_spec, is_type_wildcard, require_type_spec_arity,
};
use crate::{RuntimeError, Value};

pub(in crate::builtins::types::type_matching) fn integer_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "INTEGER", arguments, 0, 2)?;
    let lower = arguments
        .first()
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let upper = arguments
        .get(1)
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(lower.is_none_or(|bound| *number >= bound) && upper.is_none_or(|bound| *number <= bound))
}

pub(in crate::builtins::types) fn integer_type_bound(
    function: &str,
    value: &Value,
) -> Result<Option<i64>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    match value {
        Value::Integer(bound) => Ok(Some(*bound)),
        value => Err(type_error(function, "integer or *", value)),
    }
}

pub(in crate::builtins::types::type_matching) fn mod_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "MOD", arguments, 1, 1)?;
    let Value::Integer(modulus) = arguments[0] else {
        return Err(type_error(function, "non-negative integer", &arguments[0]));
    };
    if modulus < 0 {
        return Err(invalid_type_spec(
            function,
            "MOD type specifier requires a non-negative modulus",
        ));
    }
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(*number >= 0 && *number < modulus)
}

pub(in crate::builtins::types::type_matching) fn unsigned_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "UNSIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    if *number < 0 {
        return Ok(false);
    }
    let Some(size) = size else {
        return Ok(true);
    };
    if size >= 63 {
        return Ok(true);
    }
    let upper = (1_i128 << size) - 1;
    Ok(i128::from(*number) <= upper)
}

pub(in crate::builtins::types::type_matching) fn signed_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "SIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    let Some(size) = size else {
        return Ok(true);
    };
    if size == 0 {
        return Ok(false);
    }
    if size >= 64 {
        return Ok(true);
    }
    let half = 1_i128 << (size - 1);
    let number = i128::from(*number);
    Ok(number >= -half && number < half)
}

pub(in crate::builtins::types) fn byte_type_size(
    function: &str,
    operator: &str,
    arguments: &[Value],
) -> Result<Option<usize>, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 1)?;
    let Some(size) = arguments.first() else {
        return Ok(None);
    };
    if is_type_wildcard(size) {
        return Ok(None);
    }
    let Value::Integer(size) = size else {
        return Err(type_error(function, "non-negative integer or *", size));
    };
    usize::try_from(*size).map(Some).map_err(|_| {
        invalid_type_spec(
            function,
            format!("{operator} type specifier requires a non-negative size"),
        )
    })
}

pub(crate) fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
    the_check_with_environment(arguments, None)
}

pub(crate) fn the_check_in(
    arguments: &[Value],
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    the_check_with_environment(arguments, Some(environment))
}

fn the_check_with_environment(
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<Value, RuntimeError> {
    exact(arguments, "the", 2)?;
    let type_description = arguments[1].to_string();
    if type_matches_designator("the", &arguments[0], &arguments[1], environment)? {
        Ok(arguments[0].clone())
    } else {
        Err(RuntimeError::Type {
            expected: format!("the requires value of type {type_description}"),
            actual: arguments[0].type_name().to_string(),
            span: None,
        })
    }
}

fn require_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_REQUIRE_INTEGER", 1)?;
    match &arguments[0] {
        Value::Integer(_) => Ok(arguments[0].clone()),
        value => Err(RuntimeError::Type {
            expected: "INTEGER".to_string(),
            actual: value.type_name().to_string(),
            span: None,
        }),
    }
}

fn require_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_REQUIRE_LIST", 1)?;
    match &arguments[0] {
        Value::Nil | Value::List(_) => Ok(arguments[0].clone()),
        value => Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: value.type_name().to_string(),
            span: None,
        }),
    }
}

fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

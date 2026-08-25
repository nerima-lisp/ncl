use super::*;

pub(crate) fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-name", 1)?;
    let name = match &arguments[0] {
        Value::UninternedSymbol(name) => name.to_string(),
        value => {
            let name = value
                .symbol_name()
                .ok_or_else(|| type_error("symbol-name", "a symbol", &arguments[0]))?;
            let name = match package::split_symbol(name) {
                Some((_, symbol_name, _)) => symbol_name,
                None => name,
            };
            name.to_string()
        }
    };
    Ok(Value::string(name))
}

pub(crate) fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-package", 1)?;
    let package_name = match &arguments[0] {
        Value::UninternedSymbol(_) => return Ok(Value::Nil),
        Value::Keyword(_) | Value::KeywordExact(_) => KEYWORD_PACKAGE.to_string(),
        Value::Nil | Value::Boolean(_) => COMMON_LISP_PACKAGE.to_string(),
        Value::Symbol(name) | Value::SymbolExact(name) => {
            match package::split_symbol(name.as_ref()) {
                Some((package_name, _, _)) => package::normalize_package_name(package_name),
                None => package::DEFAULT_PACKAGE.to_string(),
            }
        }
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::symbol(package_name))
}

pub(crate) fn simple_condition_format_control(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-control", 1)?;
    arguments[0]
        .simple_condition_format_control()
        .map(|control| Value::string(control.to_owned()))
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-control",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

pub(crate) fn simple_condition_format_arguments(
    arguments: &[Value],
) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-arguments", 1)?;
    arguments[0]
        .simple_condition_format_arguments()
        .map(Value::list)
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-arguments",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

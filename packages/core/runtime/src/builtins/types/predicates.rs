#![allow(clippy::wildcard_imports)]
use super::*;

pub fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::List(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

pub fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

pub fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

pub fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(
        matches!(&arguments[0], Value::Vector(_))
            && arguments[0].vector_adjustable() != Some(true)
            && !arguments[0].array_has_fill_pointer().unwrap_or(false)
            && !arguments[0].is_displaced()
            && arguments[0]
                .array_element_type()
                .and_then(|element_type| element_type.symbol_name().map(str::to_ascii_uppercase))
                .is_some_and(|element_type| element_type == "T"),
    ))
}

pub fn bit_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "bit-vector-p", 1)?;
    Ok(Value::boolean(is_bit_vector(&arguments[0])))
}

pub fn simple_bit_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-bit-vector-p", 1)?;
    Ok(Value::boolean(is_simple_bit_vector(&arguments[0])))
}

fn is_bit_vector(value: &Value) -> bool {
    matches!(value, Value::Vector(items) if items.borrow().iter().all(|item| matches!(item, Value::Integer(bit) if *bit == 0 || *bit == 1)))
}

fn is_simple_bit_vector(value: &Value) -> bool {
    is_bit_vector(value)
        && value.vector_adjustable() != Some(true)
        && !value.array_has_fill_pointer().unwrap_or(false)
        && !value.is_displaced()
}

pub fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

pub fn simple_condition_format_control(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn simple_condition_format_arguments(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn condition_message(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "condition-message", 1)?;
    arguments[0]
        .condition_message()
        .map(|message| Value::string(message.to_owned()))
        .ok_or_else(|| type_error("condition-message", "CONDITION", &arguments[0]))
}

pub fn type_error_datum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-error-datum", 1)?;
    arguments[0]
        .condition_slot("TYPE-ERROR", "DATUM")
        .ok_or_else(|| type_error("type-error-datum", "TYPE-ERROR", &arguments[0]))
}

pub fn type_error_expected_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-error-expected-type", 1)?;
    arguments[0]
        .condition_slot("TYPE-ERROR", "EXPECTED-TYPE")
        .ok_or_else(|| type_error("type-error-expected-type", "TYPE-ERROR", &arguments[0]))
}

pub fn unbound_variable_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "unbound-variable-name", 1)?;
    arguments[0]
        .condition_slot("UNBOUND-VARIABLE", "NAME")
        .ok_or_else(|| type_error("unbound-variable-name", "UNBOUND-VARIABLE", &arguments[0]))
}

pub fn arithmetic_error_operation(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arithmetic-error-operation", 1)?;
    arguments[0]
        .condition_slot("ARITHMETIC-ERROR", "OPERATION")
        .ok_or_else(|| type_error("arithmetic-error-operation", "ARITHMETIC-ERROR", &arguments[0]))
}

pub fn arithmetic_error_operands(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arithmetic-error-operands", 1)?;
    arguments[0]
        .condition_slot("ARITHMETIC-ERROR", "OPERANDS")
        .ok_or_else(|| type_error("arithmetic-error-operands", "ARITHMETIC-ERROR", &arguments[0]))
}

pub fn file_error_pathname(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-error-pathname", 1)?;
    arguments[0]
        .condition_slot("FILE-ERROR", "PATHNAME")
        .ok_or_else(|| type_error("file-error-pathname", "FILE-ERROR", &arguments[0]))
}

pub fn package_error_package(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "package-error-package", 1)?;
    arguments[0]
        .condition_slot("PACKAGE-ERROR", "PACKAGE")
        .ok_or_else(|| type_error("package-error-package", "PACKAGE-ERROR", &arguments[0]))
}

pub fn stream_error_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stream-error-stream", 1)?;
    arguments[0]
        .condition_slot("STREAM-ERROR", "STREAM")
        .ok_or_else(|| type_error("stream-error-stream", "STREAM-ERROR", &arguments[0]))
}

pub fn cell_error_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cell-error-name", 1)?;
    arguments[0]
        .condition_slot("CELL-ERROR", "NAME")
        .ok_or_else(|| type_error("cell-error-name", "CELL-ERROR", &arguments[0]))
}

pub fn undefined_function_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "undefined-function-name", 1)?;
    arguments[0]
        .condition_slot("UNDEFINED-FUNCTION", "NAME")
        .ok_or_else(|| type_error("undefined-function-name", "UNDEFINED-FUNCTION", &arguments[0]))
}

pub fn unbound_slot_instance(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "unbound-slot-instance", 1)?;
    arguments[0]
        .condition_slot("UNBOUND-SLOT", "INSTANCE")
        .ok_or_else(|| type_error("unbound-slot-instance", "UNBOUND-SLOT", &arguments[0]))
}

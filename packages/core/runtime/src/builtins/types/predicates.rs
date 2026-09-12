#![allow(clippy::wildcard_imports)]
use super::*;

pub fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::Cons(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

pub fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

pub fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    let keyword = match &arguments[0] {
        Value::Keyword(_) | Value::KeywordExact(_) => true,
        Value::InternedSymbol(symbol) => symbol.keyword(),
        _ => false,
    };
    Ok(Value::boolean(keyword))
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
        Value::InternedSymbol(symbol) => {
            if symbol.keyword() {
                KEYWORD_PACKAGE.to_string()
            } else {
                return Ok(Value::Nil);
            }
        }
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
    Ok(Value::package(package_name))
}

pub fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Vector(_))))
}

pub fn bit_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "bit-vector-p", 1)?;
    Ok(Value::boolean(
        matches!(&arguments[0], Value::Vector(items) if items.snapshot().iter().all(|item| matches!(item, Value::Integer(bit) if *bit == 0 || *bit == 1))),
    ))
}

pub fn simple_bit_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-bit-vector-p", 1)?;
    Ok(Value::boolean(
        matches!(&arguments[0], Value::Vector(items) if items.snapshot().iter().all(|item| matches!(item, Value::Integer(bit) if *bit == 0 || *bit == 1))),
    ))
}

pub fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

pub fn cell_error_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cell-error-name", 1)?;
    arguments[0]
        .condition_slot("CELL-ERROR", "NAME")
        .ok_or_else(|| type_error("cell-error-name", "CELL-ERROR", &arguments[0]))
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

fn condition_slot_accessor(
    arguments: &[Value],
    function: &str,
    condition: &str,
    slot: &str,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    arguments[0]
        .condition_slot(condition, slot)
        .ok_or_else(|| type_error(function, condition, &arguments[0]))
}

pub fn condition_message(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "condition-message", 1)?;
    arguments[0]
        .condition_message()
        .map(|message| Value::string(message.to_owned()))
        .ok_or_else(|| type_error("condition-message", "CONDITION", &arguments[0]))
}

pub fn type_error_datum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(arguments, "type-error-datum", "TYPE-ERROR", "DATUM")
}

pub fn type_error_expected_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "type-error-expected-type",
        "TYPE-ERROR",
        "EXPECTED-TYPE",
    )
}

pub fn unbound_variable_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "unbound-variable-name",
        "UNBOUND-VARIABLE",
        "NAME",
    )
}

pub fn arithmetic_error_operation(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "arithmetic-error-operation",
        "ARITHMETIC-ERROR",
        "OPERATION",
    )
}

pub fn arithmetic_error_operands(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "arithmetic-error-operands",
        "ARITHMETIC-ERROR",
        "OPERANDS",
    )
}

pub fn file_error_pathname(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(arguments, "file-error-pathname", "FILE-ERROR", "PATHNAME")
}

pub fn package_error_package(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "package-error-package",
        "PACKAGE-ERROR",
        "PACKAGE",
    )
}

pub fn stream_error_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(arguments, "stream-error-stream", "STREAM-ERROR", "STREAM")
}

pub fn undefined_function_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "undefined-function-name",
        "UNDEFINED-FUNCTION",
        "NAME",
    )
}

pub fn unbound_slot_instance(arguments: &[Value]) -> Result<Value, RuntimeError> {
    condition_slot_accessor(
        arguments,
        "unbound-slot-instance",
        "UNBOUND-SLOT",
        "INSTANCE",
    )
}

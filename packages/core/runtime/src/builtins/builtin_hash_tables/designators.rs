use crate::builtins::builtin_helpers::type_error;
use crate::builtins::type_predicates::{eql_value, equalp_value};
use crate::environment::normalize_name;
use crate::{Function, RuntimeError, Value};

pub fn hash_table_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

pub fn hash_table_test_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let name = match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => normalize_name(name),
        Value::Function(function_value) => match function_value.as_ref() {
            Function::Builtin { name, .. } | Function::Primitive { name } => normalize_name(name),
            _ => {
                return Err(type_error(
                    function,
                    "named hash-table test function",
                    value,
                ));
            }
        },
        other => return Err(type_error(function, "hash-table test designator", other)),
    };
    if matches!(name.as_str(), "EQ" | "EQL" | "EQUAL" | "EQUALP") {
        Ok(name)
    } else {
        Err(RuntimeError::InvalidForm {
            message: format!("{function} :test must be EQ, EQL, EQUAL, or EQUALP, got {name}"),
            span: None,
        })
    }
}

pub fn hash_table_key_equal(test: &str, left: &Value, right: &Value) -> bool {
    match test {
        "EQ" => left.eq_value(right),
        "EQUAL" => left.equal_value(right),
        "EQUALP" => equalp_value(left, right),
        _ => eql_value(left, right),
    }
}

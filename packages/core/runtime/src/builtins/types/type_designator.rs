#![allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let type_name = match value {
        Value::Nil | Value::Boolean(false) => "NIL",
        Value::Boolean(true) => "T",
        Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::Keyword(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => name.as_ref(),
        value => return Err(type_error(function, "type designator", value)),
    };
    let type_name = type_name.rsplit("::").next().unwrap_or(type_name);
    Ok(package::normalize_symbol_name(type_name))
}

pub(super) fn known_type_name(type_name: &str, environment: &Environment) -> bool {
    is_builtin_type_name(type_name)
        || environment.lookup_class(type_name).is_some()
        || environment.lookup_structure(type_name).is_some()
}

fn is_builtin_type_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "T" | "OBJECT"
            | "NIL"
            | "NULL"
            | "BOOLEAN"
            | "NUMBER"
            | "REAL"
            | "RATIONAL"
            | "RATIO"
            | "INTEGER"
            | "FIXNUM"
            | "BIGNUM"
            | "BIT"
            | "FLOAT"
            | "CHARACTER"
            | "BASE-CHAR"
            | "STANDARD-CHAR"
            | "EXTENDED-CHAR"
            | "STRING"
            | "BASE-STRING"
            | "SIMPLE-STRING"
            | "SIMPLE-BASE-STRING"
            | "STREAM"
            | "SYMBOL"
            | "PACKAGE"
            | "ENVIRONMENT"
            | "KEYWORD"
            | "CONS"
            | "LIST"
            | "ATOM"
            | "VECTOR"
            | "SIMPLE-VECTOR"
            | "BIT-VECTOR"
            | "SIMPLE-BIT-VECTOR"
            | "ARRAY"
            | "SIMPLE-ARRAY"
            | "HASH-TABLE"
            | "CONDITION"
            | "RESTART"
            | "STRUCTURE"
            | "SEQUENCE"
            | "FUNCTION"
            | "COMPILED-FUNCTION"
            | "UNBOUND"
            | "VALUES"
            | "CLASS"
            | "STANDARD-OBJECT"
    )
}

use crate::builtins::builtin_array_helpers::dimensions_for_array;
use crate::builtins::sequence_length;
use crate::builtins::types::type_matching::cons_vector_specs::{is_bit_value, is_bit_vector_value};
use crate::{RuntimeError, Value};

pub(in crate::builtins::types::type_matching) fn type_matches(
    value: &Value,
    type_name: &str,
) -> Result<bool, RuntimeError> {
    let result = match type_name {
        "T" | "OBJECT" => true,
        "NIL" | "NULL" => matches!(value, Value::Nil | Value::Boolean(false)),
        "BOOLEAN" => matches!(value, Value::Nil | Value::Boolean(_)),
        "NUMBER" | "REAL" => matches!(
            value,
            Value::Integer(_) | Value::BigInteger(_) | Value::Rational(_) | Value::Float(_)
        ),
        "RATIONAL" => matches!(
            value,
            Value::Integer(_) | Value::BigInteger(_) | Value::Rational(_)
        ),
        "RATIO" => matches!(value, Value::Rational(_)),
        "INTEGER" => matches!(value, Value::Integer(_) | Value::BigInteger(_)),
        "FIXNUM" => matches!(value, Value::Integer(_)),
        "BIGNUM" => matches!(value, Value::BigInteger(_)),
        "BIT" => is_bit_value(value),
        "FLOAT" => matches!(value, Value::Float(_)),
        "CHARACTER" | "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => {
            matches!(value, Value::Character(_))
        }
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            matches!(value, Value::String(_))
        }
        "STREAM" => matches!(value, Value::Stream(_)),
        "RANDOM-STATE" => matches!(value, Value::RandomState(_)),
        "SYMBOL" => matches!(
            value,
            Value::Nil
                | Value::Boolean(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
        ),
        "PACKAGE" => matches!(value, Value::Package(_)),
        "ENVIRONMENT" => matches!(value, Value::Environment(_)),
        "KEYWORD" => matches!(value, Value::Keyword(_) | Value::KeywordExact(_)),
        "CONS" => matches!(value, Value::List(_) | Value::MutableCons(_) | Value::DottedList { .. }),
        "LIST" => matches!(value, Value::Nil | Value::Boolean(false) | Value::List(_) | Value::MutableCons(_)),
        "ATOM" => !matches!(value, Value::List(_) | Value::MutableCons(_) | Value::DottedList { .. }),
        "VECTOR" | "SIMPLE-VECTOR" => matches!(value, Value::Vector(_)),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => is_bit_vector_value(value),
        "ARRAY" | "SIMPLE-ARRAY" => dimensions_for_array(value).is_some(),
        "HASH-TABLE" => matches!(value, Value::HashTable { .. }),
        "CONDITION" => matches!(value, Value::Condition(_)),
        "RESTART" => matches!(value, Value::Restart(_)),
        "ERROR" | "SERIOUS-CONDITION" | "WARNING" | "SIMPLE-CONDITION" | "SIMPLE-ERROR"
        | "SIMPLE-WARNING" | "ARITHMETIC-ERROR" | "DIVISION-BY-ZERO" | "TYPE-ERROR"
        | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR" | "COMPILER-ERROR" | "FILE-ERROR"
        | "UNBOUND-VARIABLE" | "CONTROL-ERROR" => value.condition_is_type(type_name),
        "STRUCTURE" => value.structure_name().is_some(),
        "SEQUENCE" => matches!(value, Value::Boolean(false)) || sequence_length(value).is_some(),
        "FUNCTION" | "COMPILED-FUNCTION" => matches!(value, Value::Function(_)),
        "UNBOUND" => matches!(value, Value::Unbound),
        "VALUES" => matches!(value, Value::Values(_)),
        "CLASS" => matches!(value, Value::Class(_)),
        "STANDARD-OBJECT" => matches!(value, Value::Instance(_)),
        _ if value.instance_is_type(type_name) => true,
        _ if value.structure_is_type(type_name) => true,
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!("unknown type designator {type_name}"),
                span: None,
            });
        }
    };
    Ok(result)
}

#[cfg(test)]
mod tests;

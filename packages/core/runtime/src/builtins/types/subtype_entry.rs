#![allow(clippy::wildcard_imports)]
use super::*;

pub fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator)
}

pub fn subtypep_value(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    validate_subtype_designator("subtypep", subtype, environment)?;
    validate_subtype_designator("subtypep", supertype, environment)?;
    let relation = subtype_relation(subtype, supertype, environment)?;
    Ok(Value::values(vec![
        Value::boolean(relation.unwrap_or(false)),
        Value::boolean(relation.is_some()),
    ]))
}

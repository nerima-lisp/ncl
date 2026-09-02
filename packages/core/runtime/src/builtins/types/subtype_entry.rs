#![allow(clippy::wildcard_imports)]
use super::*;

pub fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator)
}

pub fn typep_value_in(
    value: &Value,
    type_designator: &Value,
    environment: &Environment,
) -> Result<bool, RuntimeError> {
    if let Ok(type_name) = crate::builtins::types::type_designator::type_designator_name(
        "typep",
        type_designator,
    ) {
        if environment.lookup_condition(&type_name).is_some() {
            return Ok(value.condition_is_type(&type_name));
        }
    }
    type_matching::type_matches_designator_in(
        "typep", value, type_designator, environment,
    )
}

pub fn subtypep_value(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    let subtype = type_matching::resolve_type_designator_in("subtypep", subtype, environment)?;
    let supertype = type_matching::resolve_type_designator_in("subtypep", supertype, environment)?;
    validate_subtype_designator("subtypep", &subtype, environment)?;
    validate_subtype_designator("subtypep", &supertype, environment)?;
    let relation = subtype_relation(&subtype, &supertype, environment)?;
    Ok(Value::values(vec![
        Value::boolean(relation.unwrap_or(false)),
        Value::boolean(relation.is_some()),
    ]))
}

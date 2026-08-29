#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn subtype_relation(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Option<bool>, RuntimeError> {
    if same_type_designator(subtype, supertype) {
        return Ok(Some(true));
    }

    if let Some((operator, arguments)) = compound_type_parts(subtype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(argument, supertype, environment)? {
                        Some(true) => {}
                        Some(false) => return Ok(Some(false)),
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "AND" => {
                for argument in arguments {
                    if subtype_relation(argument, supertype, environment)? == Some(true) {
                        return Ok(Some(true));
                    }
                }
                return Ok(None);
            }
            "MEMBER" | "EQL" => {
                let candidates = if operator == "MEMBER" {
                    arguments
                } else {
                    &arguments[..1]
                };
                let mut unknown = false;
                for candidate in candidates {
                    match type_matches_designator("subtypep", candidate, supertype) {
                        Ok(true) => {}
                        Ok(false) => return Ok(Some(false)),
                        Err(_) => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some((super_operator, super_arguments)) = compound_type_parts(supertype)
                    && super_operator == "INTEGER"
                {
                    return Ok(Some(integer_spec_is_subtype(arguments, super_arguments)?));
                }
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" | "CONS" | "VECTOR" | "SIMPLE-VECTOR"
            | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" => {
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            _ => {}
        }
    }

    if let Some((operator, arguments)) = compound_type_parts(supertype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(true) => return Ok(Some(true)),
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(false) });
            }
            "AND" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(false) => return Ok(Some(false)),
                        Some(true) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some(subtype_name) = atomic_type_name(subtype) {
                    return Ok(Some(named_integer_is_subtype(&subtype_name, arguments)?));
                }
            }
            _ => {}
        }
    }

    Ok(atomic_subtype_relation(subtype, supertype, environment))
}

fn atomic_subtype_relation(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Option<bool> {
    let subtype_name = atomic_type_name(subtype)?;
    let supertype_name = atomic_type_name(supertype)?;
    named_subtype_relation(&subtype_name, &supertype_name, environment)
}

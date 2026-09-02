#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn validate_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    match designator {
        Value::List(items) => {
            let Some(operator_value) = items.first() else {
                return Err(invalid_type_spec(
                    function,
                    "compound type designator must name an operator",
                ));
            };
            let operator = type_designator_name(function, operator_value)?;
            let arguments = &items[1..];
            match operator.as_str() {
                "OR" | "AND" => {
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "NOT" | "EQL" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    if operator == "NOT" {
                        validate_subtype_designator(function, &arguments[0], environment)?;
                    }
                }
                "MEMBER" => {}
                "INTEGER" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for bound in arguments {
                        integer_type_bound(function, bound)?;
                    }
                }
                "MOD" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    let Value::Integer(modulus) = arguments[0] else {
                        return Err(type_error(function, "non-negative integer", &arguments[0]));
                    };
                    if modulus < 0 {
                        return Err(invalid_type_spec(
                            function,
                            "MOD type specifier requires a non-negative modulus",
                        ));
                    }
                }
                "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                    byte_type_size(function, &operator, arguments)?;
                }
                "CONS" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(size) = arguments.get(1) {
                        type_spec_size(function, size)?;
                    }
                }
                "SIMPLE-VECTOR" | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 1)?;
                    if let Some(size) = arguments.first() {
                        type_spec_size(function, size)?;
                    }
                }
                "ARRAY" | "SIMPLE-ARRAY" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(dimensions) = arguments.get(1) {
                        validate_array_dimensions_spec(function, dimensions)?;
                    }
                }
                _ => {
                    return Err(invalid_type_spec(
                        function,
                        format!("unknown compound type designator {operator}"),
                    ));
                }
            }
            Ok(())
        }
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, designator)?;
            if known_type_name(&type_name, environment)
                || environment.lookup_type_alias(&type_name).is_some()
                || environment.lookup_condition(&type_name).is_some()
            {
                Ok(())
            } else {
                Err(invalid_type_spec(
                    function,
                    format!("unknown type designator {type_name}"),
                ))
            }
        }
    }
}

fn validate_element_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    if is_type_wildcard(designator) {
        Ok(())
    } else {
        validate_subtype_designator(function, designator, environment)
    }
}

fn validate_array_dimensions_spec(function: &str, dimensions: &Value) -> Result<(), RuntimeError> {
    if is_type_wildcard(dimensions) {
        return Ok(());
    }
    match dimensions {
        Value::Nil | Value::Boolean(false) => Ok(()),
        Value::Integer(rank) => usize::try_from(*rank)
            .map(|_| ())
            .map_err(|_| invalid_type_spec(function, "array rank must be non-negative")),
        Value::List(dimensions) => {
            for dimension in dimensions.iter() {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(dimension) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                if *dimension < 0 {
                    return Err(invalid_type_spec(
                        function,
                        "array dimensions must be non-negative",
                    ));
                }
            }
            Ok(())
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

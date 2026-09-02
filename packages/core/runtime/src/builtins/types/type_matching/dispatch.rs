use crate::builtins::eql_value;
use crate::builtins::types::type_designator::type_designator_name;
use crate::builtins::types::type_matching::array_specs::array_type_matches;
use crate::builtins::types::type_matching::cons_vector_specs::{
    bit_vector_type_matches, cons_type_matches, simple_vector_type_matches, vector_type_matches,
};
use crate::builtins::types::type_matching::numeric_specs::{
    integer_type_matches, mod_type_matches, signed_byte_type_matches, unsigned_byte_type_matches,
};
use crate::builtins::types::type_matching::spec_utils::{
    invalid_type_spec, require_type_spec_arity,
};
use crate::builtins::types::type_matching::type_name_table::type_matches;
use crate::{Environment, RuntimeError, Value};
use std::collections::HashSet;

pub(in crate::builtins::types) fn type_matches_designator_in(
    function: &str,
    value: &Value,
    type_designator: &Value,
    environment: &Environment,
) -> Result<bool, RuntimeError> {
    let resolved = resolve_type_designator(function, type_designator, environment, &mut HashSet::new())?;
    type_matches_designator(function, value, &resolved)
}

pub(in crate::builtins::types) fn resolve_type_designator_in(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    resolve_type_designator(function, designator, environment, &mut HashSet::new())
}

fn resolve_type_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
    active_aliases: &mut HashSet<String>,
) -> Result<Value, RuntimeError> {
    let Ok(name) = type_designator_name(function, designator) else {
        return match designator {
            Value::List(items) => resolve_compound_type_designator(
                function,
                items.as_ref(),
                environment,
                active_aliases,
            ),
            _ => Ok(designator.clone()),
        };
    };
    let Some(alias) = environment.lookup_type_alias_definition(&name) else {
        return Ok(designator.clone());
    };
    if !alias.parameters.is_empty() {
        return Err(invalid_type_spec(
            function,
            format!("type alias {name} requires {} arguments", alias.parameters.len()),
        ));
    }
    if !active_aliases.insert(name.to_string()) {
        return Err(invalid_type_spec(function, "circular type alias"));
    }
    let resolved = resolve_type_designator(function, &alias.designator, environment, active_aliases);
    active_aliases.remove(name.as_str());
    resolved
}

fn resolve_compound_type_designator(
    function: &str,
    items: &[Value],
    environment: &Environment,
    active_aliases: &mut HashSet<String>,
) -> Result<Value, RuntimeError> {
    let Some(operator) = items.first().and_then(Value::symbol_name) else {
        return Ok(Value::list(items.to_vec()));
    };
    if let Some(alias) = environment.lookup_type_alias_definition(operator) {
        if alias.parameters.len() != items.len().saturating_sub(1) {
            return Err(invalid_type_spec(function, format!("type alias {operator} expects {} arguments", alias.parameters.len())));
        }
        if !active_aliases.insert(operator.to_string()) {
            return Err(invalid_type_spec(function, "circular type alias"));
        }
        let arguments = items.iter().skip(1).cloned().collect::<Vec<_>>();
        let substituted = substitute_type_parameters(&alias.designator, &alias.parameters, &arguments);
        let resolved = resolve_type_designator(function, &substituted, environment, active_aliases);
        active_aliases.remove(operator);
        return resolved;
    }
    let type_positions = match operator {
        "OR" | "AND" | "NOT" | "CONS" => true,
        "VECTOR" | "ARRAY" | "SIMPLE-ARRAY" => true,
        _ => false,
    };
    if !type_positions {
        return Ok(Value::list(items.to_vec()));
    }
    let mut resolved = items.to_vec();
    for (index, argument) in items.iter().enumerate().skip(1) {
        let is_type_position = match operator {
            "OR" | "AND" | "NOT" | "CONS" => true,
            "VECTOR" | "ARRAY" | "SIMPLE-ARRAY" => index == 1,
            _ => false,
        };
        if is_type_position {
            resolved[index] = resolve_type_designator(
                function,
                argument,
                environment,
                active_aliases,
            )?;
        }
    }
    Ok(Value::list(resolved))
}

fn substitute_type_parameters(value: &Value, parameters: &[std::rc::Rc<str>], arguments: &[Value]) -> Value {
    if let Some(name) = value.symbol_name() {
        if let Some(index) = parameters.iter().position(|parameter| parameter.as_ref() == name) {
            return arguments[index].clone();
        }
    }
    match value {
        Value::List(items) => Value::list(items.iter().map(|item| substitute_type_parameters(item, parameters, arguments)).collect()),
        _ => value.clone(),
    }
}

pub(in crate::builtins::types) fn type_matches_designator(
    function: &str,
    value: &Value,
    type_designator: &Value,
) -> Result<bool, RuntimeError> {
    match type_designator {
        Value::List(items) => type_matches_compound(function, value, items.as_ref()),
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, type_designator)?;
            type_matches(value, &type_name)
        }
    }
}

fn type_matches_compound(
    function: &str,
    value: &Value,
    items: &[Value],
) -> Result<bool, RuntimeError> {
    let Some(operator_value) = items.first() else {
        return Err(invalid_type_spec(
            function,
            "compound type designator must name an operator",
        ));
    };
    let operator = type_designator_name(function, operator_value)?;
    let arguments = &items[1..];
    match operator.as_str() {
        "OR" => {
            for type_designator in arguments {
                if type_matches_designator(function, value, type_designator)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        "AND" => {
            for type_designator in arguments {
                if !type_matches_designator(function, value, type_designator)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "NOT" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(!type_matches_designator(function, value, &arguments[0])?)
        }
        "MEMBER" => Ok(arguments
            .iter()
            .any(|candidate| eql_value(value, candidate))),
        "EQL" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(eql_value(value, &arguments[0]))
        }
        "INTEGER" => integer_type_matches(function, value, arguments),
        "MOD" => mod_type_matches(function, value, arguments),
        "SIGNED-BYTE" => signed_byte_type_matches(function, value, arguments),
        "UNSIGNED-BYTE" => unsigned_byte_type_matches(function, value, arguments),
        "CONS" => cons_type_matches(function, value, arguments),
        "VECTOR" => vector_type_matches(function, value, arguments),
        "SIMPLE-VECTOR" => simple_vector_type_matches(function, value, arguments),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => bit_vector_type_matches(function, value, arguments),
        "ARRAY" | "SIMPLE-ARRAY" => array_type_matches(function, &operator, value, arguments),
        _ => Err(invalid_type_spec(
            function,
            format!("unknown compound type designator {operator}"),
        )),
    }
}

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
use crate::{RuntimeError, Value};

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

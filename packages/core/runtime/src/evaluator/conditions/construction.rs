use ncl_syntax::Span;

use crate::builtins;
use crate::environment::normalize_name;
use crate::{Environment, Runtime, RuntimeError, Value};

#[path = "construction/errors.rs"]
mod errors;

use errors::{display_initarg, invalid, type_error};

pub(crate) fn make_condition(
    runtime: &Runtime,
    arguments: &[Value],
    span: Span,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(RuntimeError::Arity {
            function: "make-condition".to_owned(),
            expected: "at least one".to_owned(),
            actual: arguments.len(),
        });
    }
    let initargs = &arguments[1..];
    if !initargs.len().is_multiple_of(2) {
        return Err(invalid(
            "make-condition initargs must be keyword/value pairs",
            span,
        ));
    }
    let actual_type = runtime.name_designator_from_value(&arguments[0], span)?;
    let definition = environment.lookup_condition(&actual_type);
    let mut format_control = None;
    let mut format_arguments = Vec::new();
    let mut standard_slots = Vec::new();
    let mut supplied = Vec::new();
    for pair in initargs.chunks_exact(2) {
        let (initarg, escaped) = runtime.name_designator_info_from_value(&pair[0], span)?;
        let initarg_name = if escaped {
            initarg.clone()
        } else {
            normalize_name(&initarg)
        };
        match (escaped, initarg_name.as_str()) {
            (false, "FORMAT-CONTROL") => {
                let Value::String(control) = &pair[1] else {
                    return Err(type_error("STRING", &pair[1], span));
                };
                format_control = Some(control.to_string());
            }
            (false, "FORMAT-ARGUMENTS") => {
                format_arguments = pair[1]
                    .list_items()
                    .ok_or_else(|| type_error("PROPER-LIST", &pair[1], span))?;
            }
            (_, "DATUM" | "EXPECTED-TYPE")
                if actual_type.eq_ignore_ascii_case("TYPE-ERROR")
                    || actual_type.eq_ignore_ascii_case("SIMPLE-TYPE-ERROR") =>
            {
                standard_slots.push((initarg_name, pair[1].clone()));
            }
            _ => {
                supplied.push((initarg, escaped, pair[1].clone()));
            }
        }
    }
    if let Some(definition) = &definition {
        for (initarg, escaped, _) in &supplied {
            if !definition.slots.iter().any(|slot| {
                slot.initarg
                    .as_ref()
                    .is_some_and(|name| name.matches(initarg, *escaped))
            }) {
                return Err(invalid(
                    format!(
                        "unknown make-condition initarg {}",
                        display_initarg(initarg, *escaped)
                    ),
                    span,
                ));
            }
        }
        return build_defined_condition(
            runtime,
            definition,
            &supplied,
            format_control,
            format_arguments,
            environment,
        );
    }
    if let Some((initarg, escaped, _)) = supplied.first() {
        return Err(invalid(
            format!(
                "unknown make-condition initarg {}",
                display_initarg(initarg, *escaped)
            ),
            span,
        ));
    }
    let message = match format_control.as_deref() {
        Some(control) => builtins::format_control(control, &format_arguments)?,
        None => String::new(),
    };
    Ok(Value::condition_from_parts_with_slots(
        actual_type,
        standard_slots,
        message,
        format_control,
        format_arguments,
    ))
}

fn build_defined_condition(
    runtime: &Runtime,
    definition: &crate::value::ConditionDefinition,
    supplied: &[(String, bool, Value)],
    format_control: Option<String>,
    format_arguments: Vec<Value>,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    let mut slots = Vec::new();
    for slot in &definition.slots {
        let value = match slot.initarg.as_ref().and_then(|name| {
            supplied
                .iter()
                .rev()
                .find(|(initarg, escaped, _)| name.matches(initarg, *escaped))
                .map(|(_, _, value)| value)
        }) {
            Some(value) => value.clone(),
            None => match &slot.init_form {
                Some(form) => runtime.eval_in(form, environment)?,
                None => Value::Unbound,
            },
        };
        slots.push((slot.name.clone(), value));
    }
    let message = match format_control.as_deref() {
        Some(control) => builtins::format_control(control, &format_arguments)?,
        None => definition.report.clone().unwrap_or_default(),
    };
    Ok(Value::condition_from_definition(
        definition.name.clone(),
        definition.precedence.clone(),
        slots,
        message,
        format_control,
        format_arguments,
    ))
}

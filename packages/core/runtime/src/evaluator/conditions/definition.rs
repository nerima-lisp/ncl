use std::rc::Rc;

use ncl_syntax::{Form, FormKind, Span};

use crate::environment::normalize_name;
use crate::value::ConditionDefinition;
use crate::{Environment, Runtime, RuntimeError, Value};

#[path = "definition/hierarchy.rs"]
mod hierarchy;
#[path = "definition/slots.rs"]
mod slots;

pub(crate) fn define_condition(
    runtime: &Runtime,
    items: &[Form],
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    if items.len() < 4 {
        return Err(invalid(
            "define-condition requires a name, superclass list, and slot list",
            items.first().map_or(Span::new(0, 0), |item| item.span),
        ));
    }
    let name = runtime.definition_name_from_form(&items[1], "condition name must be a symbol")?;
    let superclass_forms =
        runtime.list_form_items(&items[2], "condition superclasses must be a list")?;
    let direct_superclasses = if superclass_forms.is_empty() {
        vec!["CONDITION".to_owned()]
    } else {
        superclass_forms
            .iter()
            .map(|form| {
                runtime.definition_name_from_form(form, "condition superclass must be a symbol")
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let slot_forms = runtime.list_form_items(&items[3], "condition slots must be a list")?;
    let mut slots = slot_forms
        .iter()
        .map(|form| slots::parse_slot(runtime, form))
        .collect::<Result<Vec<_>, _>>()?;
    let report = parse_options(runtime, &items[4..])?;
    let precedence = hierarchy::condition_precedence(&name, &direct_superclasses, environment);
    slots = hierarchy::inherited_slots(&precedence, &slots, environment);
    let definition = Rc::new(ConditionDefinition {
        name: name.clone(),
        direct_superclasses,
        precedence,
        slots: slots.clone(),
        report,
    });
    for slot in &slots {
        for reader in &slot.readers {
            environment.define_function(
                reader,
                Value::condition_reader(name.clone(), slot.name.clone()),
            );
        }
        for writer in &slot.writers {
            environment.define_function(
                writer,
                Value::condition_writer(name.clone(), slot.name.clone()),
            );
        }
    }
    environment.define_condition(name.clone(), definition);
    Ok(Value::symbol(name))
}

fn parse_options(runtime: &Runtime, options: &[Form]) -> Result<Option<String>, RuntimeError> {
    let mut report = None;
    for option in options {
        let items = runtime.list_form_items(option, "condition option must be a list")?;
        if items.is_empty() {
            return Err(invalid("condition option cannot be empty", option.span));
        }
        let name = runtime
            .definition_name_from_form(&items[0], "condition option name must be a symbol")?;
        match name.as_str() {
            "REPORT" => {
                if items.len() != 2 {
                    return Err(invalid(
                        "condition report option requires one value",
                        option.span,
                    ));
                }
                report = match &items[1].kind {
                    FormKind::String(value) => Some(value.clone()),
                    FormKind::Atom(value) if normalize_name(value) == "NIL" => None,
                    _ => {
                        return Err(invalid(
                            "condition report must be a string or NIL",
                            items[1].span,
                        ));
                    }
                };
            }
            "DOCUMENTATION" => {
                if items.len() != 2 {
                    return Err(invalid(
                        "condition documentation option requires one value",
                        option.span,
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    format!("unknown condition option :{name}"),
                    option.span,
                ));
            }
        }
    }
    Ok(report)
}

fn invalid(message: impl Into<String>, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.into(),
        span: Some(span),
    }
}

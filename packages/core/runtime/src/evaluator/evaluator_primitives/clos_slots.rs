#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn slot_definition_value(slot: &ClassSlot) -> Value {
        let class = Rc::new(ClassDefinition {
            name: "STANDARD-DIRECT-SLOT-DEFINITION".to_owned(),
            documentation: None,
            direct_superclasses: vec!["SLOT-DEFINITION".into()],
            direct_slots: Vec::new(),
            direct_default_initargs: Vec::new(),
            precedence: vec![
                "STANDARD-DIRECT-SLOT-DEFINITION".into(),
                "SLOT-DEFINITION".into(),
                "STANDARD-OBJECT".into(),
            ],
            slots: Vec::new(),
            default_initargs: Vec::new(),
        });
        Value::instance(
            class,
            vec![
                (
                    "NAME".to_owned(),
                    Value::symbol(Rc::from(slot.name.clone())),
                ),
                (
                    "DOCUMENTATION".to_owned(),
                    slot.documentation
                        .as_ref()
                        .map_or(Value::Nil, |value| Value::string(value.clone())),
                ),
                (
                    "INITARGS".to_owned(),
                    Value::list(
                        slot.initargs
                            .iter()
                            .map(|name| Value::keyword(name.clone()))
                            .collect(),
                    ),
                ),
                (
                    "ALLOCATION".to_owned(),
                    Value::keyword(if slot.class_value.is_some() {
                        "CLASS"
                    } else {
                        "INSTANCE"
                    }),
                ),
            ],
        )
    }

    pub(crate) fn apply_slot_definition_primitive(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "SLOT-DEFINITION-NAME"
                | "SLOT-DEFINITION-DOCUMENTATION"
                | "SLOT-DEFINITION-INITARGS"
                | "SLOT-DEFINITION-ALLOCATION"
        ) {
            return None;
        }
        Some((|| {
            if arguments.len() != 1 {
                return Err(Self::arity(
                    "slot-definition operation",
                    "one",
                    arguments.len(),
                ));
            }
            if !arguments[0].instance_is_type("SLOT-DEFINITION") {
                return Err(RuntimeError::Type {
                    expected: "SLOT-DEFINITION".to_owned(),
                    actual: arguments[0].type_name().to_owned(),
                    span: Some(span),
                });
            }
            let slot_name = match name {
                "SLOT-DEFINITION-NAME" => "NAME",
                "SLOT-DEFINITION-DOCUMENTATION" => "DOCUMENTATION",
                "SLOT-DEFINITION-INITARGS" => "INITARGS",
                _ => "ALLOCATION",
            };
            arguments[0]
                .instance_slot(slot_name)
                .ok_or_else(|| Self::invalid("slot definition has no requested slot", span))
        })())
    }

    pub(crate) fn apply_slot_primitive(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "SLOT-VALUE" | "SLOT-EXISTS-P" | "SLOT-BOUNDP" | "SLOT-MAKUNBOUND"
        ) {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            if arguments.len() != 2 {
                return Err(Self::arity("slot operation", "two", arguments.len()));
            }
            let slot_name = Self::slot_name_from_value(&arguments[1], span)?;
            if !matches!(arguments[0], Value::Instance(_)) {
                return Err(RuntimeError::Type {
                    expected: "STANDARD-OBJECT".to_owned(),
                    actual: arguments[0].type_name().to_string(),
                    span: Some(span),
                });
            }
            match name {
                "SLOT-VALUE" => {
                    let value = arguments[0]
                        .instance_slot(&slot_name)
                        .ok_or_else(|| Self::invalid("slot is not defined for this class", span))?;
                    if matches!(value, Value::Unbound) {
                        return Err(Self::invalid("slot is unbound", span));
                    }
                    Ok(value)
                }
                "SLOT-EXISTS-P" => Ok(Value::boolean(
                    arguments[0].instance_slot_exists(&slot_name),
                )),
                "SLOT-BOUNDP" => Ok(Value::boolean(
                    arguments[0]
                        .instance_slot_is_bound(&slot_name)
                        .unwrap_or(false),
                )),
                "SLOT-MAKUNBOUND" => {
                    let Some(class) = arguments[0].instance_class_definition() else {
                        return Err(RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: arguments[0].type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    if !arguments[0].instance_slot_exists(&slot_name)
                        || !arguments[0].set_instance_slot(&class.name, &slot_name, Value::Unbound)
                    {
                        return Err(Self::invalid("slot is not defined for this class", span));
                    }
                    Ok(arguments[0].clone())
                }
                _ => unreachable!("slot primitive name was prevalidated"),
            }
        })();
        Some(result)
    }
}

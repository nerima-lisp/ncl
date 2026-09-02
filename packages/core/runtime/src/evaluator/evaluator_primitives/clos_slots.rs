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
            class.clone(),
            vec![("CLASS".to_owned(), Value::class_object(class)),
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
                    "READERS".to_owned(),
                    Value::list(
                        slot.readers
                            .iter()
                            .map(|name| Value::symbol(Rc::from(name.clone())))
                            .collect(),
                    ),
                ),
                (
                    "WRITERS".to_owned(),
                    Value::list(
                        slot.writers
                            .iter()
                            .map(|name| Value::symbol(Rc::from(name.clone())))
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
                (
                    "INITFORM".to_owned(),
                    slot.init_form
                        .as_ref()
                        .map_or(Ok(Value::Nil), quoted_form_value)
                        .expect("slot initform is always quoteable"),
                ),
                (
                    "INITFUNCTION".to_owned(),
                    slot.init_function.clone().unwrap_or(Value::Nil),
                ),
                (
                    "TYPE".to_owned(),
                    slot.type_form
                        .as_ref()
                        .map_or(Ok(Value::symbol("T")), quoted_form_value)
                        .expect("slot type is always quoteable"),
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
                | "SLOT-DEFINITION-CLASS"
                | "SLOT-DEFINITION-DOCUMENTATION"
                | "SLOT-DEFINITION-INITARGS"
                | "SLOT-DEFINITION-ALLOCATION"
                | "SLOT-DEFINITION-INITFORM"
                | "SLOT-DEFINITION-INITFUNCTION"
                | "SLOT-DEFINITION-TYPE"
                | "SLOT-DEFINITION-READERS"
                | "SLOT-DEFINITION-WRITERS"
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
                "SLOT-DEFINITION-CLASS" => "CLASS",
                "SLOT-DEFINITION-DOCUMENTATION" => "DOCUMENTATION",
                "SLOT-DEFINITION-INITARGS" => "INITARGS",
                "SLOT-DEFINITION-ALLOCATION" => "ALLOCATION",
                "SLOT-DEFINITION-INITFORM" => "INITFORM",
                "SLOT-DEFINITION-INITFUNCTION" => "INITFUNCTION",
                "SLOT-DEFINITION-TYPE" => "TYPE",
                "SLOT-DEFINITION-READERS" => "READERS",
                _ => "WRITERS",
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
            "SLOT-VALUE"
                | "SLOT-EXISTS-P"
                | "SLOT-BOUNDP"
                | "SLOT-MAKUNBOUND"
                | "SLOT-VALUE-USING-CLASS"
                | "SLOT-EXISTS-P-USING-CLASS"
                | "SLOT-BOUNDP-USING-CLASS"
                | "SLOT-MAKUNBOUND-USING-CLASS"
        ) {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            let using_class = name.ends_with("-USING-CLASS");
            if arguments.len() != if using_class { 3 } else { 2 } {
                return Err(Self::arity(
                    "slot operation",
                    if using_class { "three" } else { "two" },
                    arguments.len(),
                ));
            }
            let (object, slot_argument, expected_class) = if using_class {
                let Some(expected_class) = arguments[0].class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "CLASS".to_owned(),
                        actual: arguments[0].type_name().to_owned(),
                        span: Some(span),
                    });
                };
                (&arguments[1], &arguments[2], Some(expected_class))
            } else {
                (&arguments[0], &arguments[1], None)
            };
            let slot_name = Self::slot_name_from_value(slot_argument, span)?;
            if !matches!(object, Value::Instance(_)) {
                return Err(RuntimeError::Type {
                    expected: "STANDARD-OBJECT".to_owned(),
                    actual: arguments[0].type_name().to_string(),
                    span: Some(span),
                });
            }
            if let Some(expected_class) = expected_class {
                let actual_class = object
                    .instance_class_definition()
                    .ok_or_else(|| Self::invalid("object has no class definition", span))?;
                if !actual_class
                    .precedence
                    .iter()
                    .any(|name| name.as_ref() == expected_class.name)
                {
                    return Err(Self::invalid("class is not a superclass of object", span));
                }
            }
            match name {
                "SLOT-VALUE" | "SLOT-VALUE-USING-CLASS" => {
                    let value = object
                        .instance_slot(&slot_name)
                        .ok_or_else(|| Self::invalid("slot is not defined for this class", span))?;
                    if matches!(value, Value::Unbound) {
                        return Err(RuntimeError::UnboundSlot {
                            name: slot_name.clone(),
                            span: Some(span),
                        });
                    }
                    Ok(value)
                }
                "SLOT-EXISTS-P" | "SLOT-EXISTS-P-USING-CLASS" => {
                    Ok(Value::boolean(object.instance_slot_exists(&slot_name)))
                }
                "SLOT-BOUNDP" | "SLOT-BOUNDP-USING-CLASS" => Ok(Value::boolean(
                    object.instance_slot_is_bound(&slot_name).unwrap_or(false),
                )),
                "SLOT-MAKUNBOUND" | "SLOT-MAKUNBOUND-USING-CLASS" => {
                    let Some(class) = object.instance_class_definition() else {
                        return Err(RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: arguments[0].type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    if !object.instance_slot_exists(&slot_name)
                        || !object.set_instance_slot(&class.name, &slot_name, Value::Unbound)
                    {
                        return Err(Self::invalid("slot is not defined for this class", span));
                    }
                    Ok(object.clone())
                }
                _ => unreachable!("slot primitive name was prevalidated"),
            }
        })();
        Some(result)
    }
}

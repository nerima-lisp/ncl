#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
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

    pub(crate) fn apply_class_introspection_primitive(
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "SUBTYPEP" | "CLASS-OF" | "FIND-CLASS" | "CLASS-NAME") {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "SUBTYPEP" => {
                    if arguments.len() != 2 {
                        return Err(Self::arity("subtypep", "two", arguments.len()));
                    }
                    builtins::subtypep_value(&arguments[0], &arguments[1], environment)
                }
                "CLASS-OF" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-of", "one", arguments.len()));
                    }
                    let class = if let Value::Instance(instance) = &arguments[0] {
                        instance.class.clone()
                    } else {
                        let n = arguments[0].type_name().to_owned();
                        Rc::new(ClassDefinition {
                            name: n.clone(),
                            precedence: vec![n.into(), "STANDARD-OBJECT".into()],
                            slots: Vec::new(),
                            default_initargs: Vec::new(),
                        })
                    };
                    Ok(Value::class_object(class))
                }
                "FIND-CLASS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-class", "one", arguments.len()));
                    }
                    let n = Self::name_designator_from_value(&arguments[0], span)?;
                    Ok(Value::class_object(
                        environment
                            .lookup_class(&n)
                            .ok_or_else(|| Self::invalid("unknown class", span))?,
                    ))
                }
                "CLASS-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-name", "one", arguments.len()));
                    }
                    let Value::Class(c) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(Value::symbol(c.name.clone()))
                }
                _ => unreachable!("class introspection primitive name was prevalidated"),
            }
        })())
    }
}

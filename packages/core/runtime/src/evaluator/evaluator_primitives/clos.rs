#![allow(clippy::wildcard_imports)]
use super::*;
use crate::Function;

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
        if !matches!(
            name,
            "SUBTYPEP"
                | "CLASS-OF"
                | "FIND-CLASS"
                | "CLASS-NAME"
                | "CLASS-PRECEDENCE-LIST"
                | "CLASS-DIRECT-SUPERCLASSES"
                | "CLASS-DIRECT-SLOTS"
                | "CLASS-SLOTS"
                | "CLASS-DEFAULT-INITARGS"
                | "CLASS-DIRECT-DEFAULT-INITARGS"
                | "CLASS-FINALIZED-P"
                | "FINALIZE-INHERITANCE"
                | "GENERIC-FUNCTION-NAME"
        ) {
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
                            direct_superclasses: vec!["STANDARD-OBJECT".into()],
                            direct_slots: Vec::new(),
                            direct_default_initargs: Vec::new(),
                            precedence: vec![n.into(), "STANDARD-OBJECT".into()],
                            slots: Vec::new(),
                            default_initargs: Vec::new(),
                        })
                    };
                    Ok(Value::class_object(class))
                }
                "FIND-CLASS" => {
                    if !(1..=2).contains(&arguments.len()) {
                        return Err(Self::arity("find-class", "one or two", arguments.len()));
                    }
                    let n = Self::name_designator_from_value(&arguments[0], span)?;
                    match environment.lookup_class(&n) {
                        Some(class) => Ok(Value::class_object(class)),
                        None if arguments.get(1).is_some_and(|errorp| !errorp.is_truthy()) => {
                            Ok(Value::Nil)
                        }
                        None => Err(Self::invalid("unknown class", span)),
                    }
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
                "GENERIC-FUNCTION-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("generic-function-name", "one", arguments.len()));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { name, .. } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(Value::symbol(name.clone()))
                }
                "CLASS-PRECEDENCE-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-precedence-list", "one", arguments.len()));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let classes = class.precedence.iter().map(|name| {
                        environment.lookup_class(name).unwrap_or_else(|| {
                            Rc::new(ClassDefinition {
                                name: name.to_string(),
                                direct_superclasses: Vec::new(),
                                direct_slots: Vec::new(),
                                direct_default_initargs: Vec::new(),
                                precedence: vec![name.clone()],
                                slots: Vec::new(),
                                default_initargs: Vec::new(),
                            })
                        })
                    });
                    Ok(Value::list(classes.map(Value::class_object).collect()))
                }
                "CLASS-DIRECT-SUPERCLASSES" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "class-direct-superclasses",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let classes = class.direct_superclasses.iter().map(|name| {
                        environment.lookup_class(name).unwrap_or_else(|| {
                            Rc::new(ClassDefinition {
                                name: name.to_string(),
                                direct_superclasses: Vec::new(),
                                direct_slots: Vec::new(),
                                direct_default_initargs: Vec::new(),
                                precedence: vec![name.clone()],
                                slots: Vec::new(),
                                default_initargs: Vec::new(),
                            })
                        })
                    });
                    Ok(Value::list(classes.map(Value::class_object).collect()))
                }
                "CLASS-DIRECT-SLOTS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-direct-slots", "one", arguments.len()));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(Value::list(
                        class
                            .direct_slots
                            .iter()
                            .map(|name| Value::symbol(name.clone()))
                            .collect(),
                    ))
                }
                "CLASS-SLOTS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-slots", "one", arguments.len()));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(Value::list(
                        class
                            .slots
                            .iter()
                            .map(|slot| Value::symbol(Rc::<str>::from(slot.name.clone())))
                            .collect(),
                    ))
                }
                "CLASS-DEFAULT-INITARGS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "class-default-initargs",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let initargs = class
                        .default_initargs
                        .iter()
                        .map(|(name, form)| {
                            Ok(Value::cons_cell(
                                Value::symbol(name.clone()),
                                quoted_form_value(form)?,
                            ))
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?;
                    Ok(Value::list(initargs))
                }
                "CLASS-DIRECT-DEFAULT-INITARGS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "class-direct-default-initargs",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Class(class) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let initargs = class
                        .direct_default_initargs
                        .iter()
                        .map(|(name, form)| {
                            Ok(Value::cons_cell(
                                Value::symbol(name.clone()),
                                quoted_form_value(form)?,
                            ))
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?;
                    Ok(Value::list(initargs))
                }
                "CLASS-FINALIZED-P" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-finalized-p", "one", arguments.len()));
                    }
                    if !matches!(arguments[0], Value::Class(_)) {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    }
                    Ok(Value::boolean(true))
                }
                "FINALIZE-INHERITANCE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("finalize-inheritance", "one", arguments.len()));
                    }
                    if !matches!(arguments[0], Value::Class(_)) {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".to_owned(),
                            actual: arguments[0].type_name().to_string(),
                            span: Some(span),
                        });
                    }
                    Ok(arguments[0].clone())
                }
                _ => unreachable!("class introspection primitive name was prevalidated"),
            }
        })())
    }
}

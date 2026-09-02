#![allow(clippy::wildcard_imports)]
use super::*;
use crate::value::MethodCombination;
use crate::value::MethodSpecializer;
use crate::Function;

impl Runtime {
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
                | "CLASS-DOCUMENTATION"
                | "CLASS-PRECEDENCE-LIST"
                | "CLASS-DIRECT-SUPERCLASSES"
                | "CLASS-DIRECT-SLOTS"
                | "CLASS-SLOTS"
                | "CLASS-DEFAULT-INITARGS"
                | "CLASS-DIRECT-DEFAULT-INITARGS"
                | "CLASS-FINALIZED-P"
                | "FINALIZE-INHERITANCE"
                | "GENERIC-FUNCTION-NAME"
                | "GENERIC-FUNCTION-METHOD-COMBINATION"
                | "GENERIC-FUNCTION-LAMBDA-LIST"
                | "GENERIC-FUNCTION-DOCUMENTATION"
                | "GENERIC-FUNCTION-METHODS"
                | "ENSURE-GENERIC-FUNCTION"
                | "FIND-METHOD"
                | "ADD-METHOD"
                | "REMOVE-METHOD"
                | "METHOD-QUALIFIERS"
                | "METHOD-SPECIALIZERS"
                | "METHOD-FUNCTION"
        ) {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "ENSURE-GENERIC-FUNCTION" => {
                    if arguments.is_empty() {
                        return Err(Self::arity("ensure-generic-function", "at least one", 0));
                    }
                    let name = Self::name_designator_from_value(&arguments[0], span)?;
                    if arguments.len() % 2 == 0 {
                        return Err(Self::invalid("keyword arguments must be paired", span));
                    }
                    let mut lambda_list = None;
                    let mut method_combination = MethodCombination::Standard;
                    let mut documentation = None;
                    let mut index = 1;
                    while index < arguments.len() {
                        let key = Self::name_designator_from_value(&arguments[index], span)?;
                        let value = &arguments[index + 1];
                        match key.as_str() {
                            "LAMBDA-LIST" => {
                                lambda_list = Some(Self::form_from_value(value, span)?)
                            }
                            "METHOD-COMBINATION" => {
                                let combination = Self::name_designator_from_value(value, span)?;
                                method_combination = match combination.as_str() {
                                    "STANDARD" => MethodCombination::Standard,
                                    "AND" => MethodCombination::And,
                                    "OR" => MethodCombination::Or,
                                    "PROGN" => MethodCombination::Progn,
                                    "LIST" => MethodCombination::List,
                                    "APPEND" => MethodCombination::Append,
                                    "NCONC" => MethodCombination::Nconc,
                                    "+" => MethodCombination::Plus,
                                    "MAX" => MethodCombination::Max,
                                    "MIN" => MethodCombination::Min,
                                    _ => {
                                        return Err(Self::invalid(
                                            "unknown method combination",
                                            span,
                                        ))
                                    }
                                };
                            }
                            "DOCUMENTATION" => {
                                let Value::String(value) = value else {
                                    return Err(RuntimeError::Type {
                                        expected: "STRING".into(),
                                        actual: value.type_name().into(),
                                        span: Some(span),
                                    });
                                };
                                documentation = Some(value.to_string());
                            }
                            "ARGUMENT-PRECEDENCE-ORDER"
                            | "DECLARATIONS"
                            | "GENERIC-FUNCTION-CLASS"
                            | "METHOD-CLASS" => {}
                            _ => {
                                return Err(Self::invalid(
                                    "unknown ensure-generic-function option",
                                    span,
                                ))
                            }
                        }
                        index += 2;
                    }
                    if let Some(function) = environment.lookup_function(&name) {
                        if matches!(function, Value::Function(ref f) if matches!(f.as_ref(), Function::Generic { .. }))
                        {
                            return Ok(function);
                        }
                        return Err(Self::invalid("function name is already defined", span));
                    }
                    let function = match lambda_list {
                        Some(form) => Value::generic_with_lambda_list(
                            name.clone(),
                            form,
                            method_combination,
                            documentation,
                        ),
                        None => Value::generic_with_combination(
                            name.clone(),
                            method_combination,
                            documentation,
                        ),
                    };
                    environment.define_function(&name, function.clone());
                    Ok(function)
                }
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
                            documentation: None,
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
                "CLASS-DOCUMENTATION" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("class-documentation", "one", arguments.len()));
                    }
                    let Value::Class(c) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "CLASS".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(c.documentation
                        .as_ref()
                        .map_or(Value::Nil, |value| Value::string(value.clone())))
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
                "GENERIC-FUNCTION-METHOD-COMBINATION" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "generic-function-method-combination",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic {
                        method_combination, ..
                    } = function.as_ref()
                    else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let name = match method_combination {
                        MethodCombination::Standard => "STANDARD",
                        MethodCombination::And => "AND",
                        MethodCombination::Or => "OR",
                        MethodCombination::Progn => "PROGN",
                        MethodCombination::List => "LIST",
                        MethodCombination::Append => "APPEND",
                        MethodCombination::Nconc => "NCONC",
                        MethodCombination::Plus => "+",
                        MethodCombination::Max => "MAX",
                        MethodCombination::Min => "MIN",
                    };
                    Ok(Value::symbol(name))
                }
                "GENERIC-FUNCTION-LAMBDA-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "generic-function-lambda-list",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { lambda_list, .. } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(lambda_list
                        .as_ref()
                        .map(quoted_form_value)
                        .transpose()?
                        .unwrap_or(Value::Nil))
                }
                "GENERIC-FUNCTION-DOCUMENTATION" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "generic-function-documentation",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { documentation, .. } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(documentation
                        .as_deref()
                        .map(Value::string)
                        .unwrap_or(Value::Nil))
                }
                "GENERIC-FUNCTION-METHODS" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity(
                            "generic-function-methods",
                            "one",
                            arguments.len(),
                        ));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { methods, .. } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    Ok(Value::list(
                        methods
                            .borrow()
                            .iter()
                            .cloned()
                            .map(Value::method)
                            .collect(),
                    ))
                }
                "FIND-METHOD" => {
                    if arguments.len() != 3 {
                        return Err(Self::arity("find-method", "three", arguments.len()));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { methods, .. } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let qualifiers =
                        arguments[1]
                            .list_items()
                            .ok_or_else(|| RuntimeError::Type {
                                expected: "LIST".into(),
                                actual: arguments[1].type_name().into(),
                                span: Some(span),
                            })?;
                    let specializers =
                        arguments[2]
                            .list_items()
                            .ok_or_else(|| RuntimeError::Type {
                                expected: "LIST".into(),
                                actual: arguments[2].type_name().into(),
                                span: Some(span),
                            })?;
                    let qualifier_names = qualifiers
                        .iter()
                        .map(|value| Self::name_designator_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?;
                    let methods = methods.borrow();
                    let matches = methods.iter().find(|method| {
                        method.qualifiers == qualifier_names
                            && method.specializers.len() == specializers.len()
                            && method.specializers.iter().zip(&specializers).all(
                                |(method_specializer, requested)| match method_specializer {
                                    crate::value::MethodSpecializer::Class(name) => {
                                        Self::name_designator_from_value(requested, span)
                                            .is_ok_and(|requested| requested == name.as_ref())
                                    }
                                    crate::value::MethodSpecializer::Eql(value) => {
                                        crate::builtins::eql_value(value, requested)
                                    }
                                },
                            )
                    });
                    Ok(matches.cloned().map(Value::method).unwrap_or(Value::Nil))
                }
                "ADD-METHOD" | "REMOVE-METHOD" => {
                    if arguments.len() != 2 {
                        return Err(Self::arity("method mutation", "two", arguments.len()));
                    }
                    let Value::Function(generic_function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Generic { methods, .. } = generic_function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "GENERIC-FUNCTION".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Value::Function(method_function) = &arguments[1] else {
                        return Err(RuntimeError::Type {
                            expected: "METHOD".into(),
                            actual: arguments[1].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Method { definition } = method_function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "METHOD".into(),
                            actual: arguments[1].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let same_signature = |candidate: &MethodDefinition| {
                        candidate.qualifiers == definition.qualifiers
                            && candidate.specializers.len() == definition.specializers.len()
                            && candidate
                                .specializers
                                .iter()
                                .zip(&definition.specializers)
                                .all(|(left, right)| match (left, right) {
                                    (
                                        MethodSpecializer::Class(left),
                                        MethodSpecializer::Class(right),
                                    ) => left == right,
                                    (
                                        MethodSpecializer::Eql(left),
                                        MethodSpecializer::Eql(right),
                                    ) => builtins::eql_value(left, right),
                                    _ => false,
                                })
                    };
                    let mut methods = methods.borrow_mut();
                    match name {
                        "ADD-METHOD" => {
                            if let Some(index) = methods.iter().position(same_signature) {
                                methods[index] = definition.clone();
                            } else {
                                methods.push(definition.clone());
                            }
                        }
                        "REMOVE-METHOD" => {
                            if let Some(index) = methods.iter().position(|candidate| {
                                same_signature(candidate)
                                    && candidate.function.eq_value(&definition.function)
                            }) {
                                methods.remove(index);
                            }
                        }
                        _ => unreachable!(),
                    }
                    Ok(arguments[0].clone())
                }
                "METHOD-QUALIFIERS" | "METHOD-SPECIALIZERS" | "METHOD-FUNCTION" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("method introspection", "one", arguments.len()));
                    }
                    let Value::Function(function) = &arguments[0] else {
                        return Err(RuntimeError::Type {
                            expected: "METHOD".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    let Function::Method { definition } = function.as_ref() else {
                        return Err(RuntimeError::Type {
                            expected: "METHOD".into(),
                            actual: arguments[0].type_name().into(),
                            span: Some(span),
                        });
                    };
                    match name {
                        "METHOD-QUALIFIERS" => Ok(Value::list(
                            definition.qualifiers.iter().map(Value::symbol).collect(),
                        )),
                        "METHOD-SPECIALIZERS" => Ok(Value::list(
                            definition
                                .specializers
                                .iter()
                                .map(|specializer| match specializer {
                                    crate::value::MethodSpecializer::Class(name) => {
                                        Value::symbol(name.as_ref())
                                    }
                                    crate::value::MethodSpecializer::Eql(value) => value.clone(),
                                })
                                .collect(),
                        )),
                        "METHOD-FUNCTION" => Ok(definition.function.clone()),
                        _ => unreachable!(),
                    }
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
                                documentation: None,
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
                                documentation: None,
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
                            .slots
                            .iter()
                            .filter(|slot| {
                                class
                                    .direct_slots
                                    .iter()
                                    .any(|name| name.eq_ignore_ascii_case(&slot.name))
                            })
                            .map(|slot| Self::slot_definition_value(slot, environment))
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
                            .map(|slot| Self::slot_definition_value(slot, environment))
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

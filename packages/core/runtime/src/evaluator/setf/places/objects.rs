impl Runtime {
    fn set_object_place(
        &self,
        operator: &str,
        args: &[Form],
        value: &Value,
        place: &Form,
        environment: &Environment,
    ) -> Option<Result<(), RuntimeError>> {
        Some(match operator {
            "SLOT-VALUE" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf slot-value", "two", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let slot = match self.eval_in(&args[1], environment) {
                    Ok(slot) => slot,
                    Err(error) => return Some(Err(error)),
                };
                let slot_name = match self.slot_name_from_value(&slot, place.span) {
                    Ok(slot_name) => slot_name,
                    Err(error) => return Some(Err(error)),
                };
                let Some(class) = current.instance_class_definition() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                if current.set_instance_slot(&class.name, &slot_name, value.clone()) {
                    Ok(())
                } else {
                    match self.slot_missing(
                        class,
                        &current,
                        &slot_name,
                        "SETF",
                        Some(value.clone()),
                        EvaluationContext {
                            environment,
                            span: place.span,
                        },
                    ) {
                        Ok(_) => Ok(()),
                        Err(error) => Err(error),
                    }
                }
            }
            "THE" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf THE", "two", args.len())));
                }
                let type_designator = match quoted_form_value(&args[0]) {
                    Ok(type_designator) => type_designator,
                    Err(error) => return Some(Err(error)),
                };
                let checked = match builtins::the_check_in(
                    &[value.clone(), type_designator],
                    environment,
                ) {
                    Ok(checked) => checked,
                    Err(error) => return Some(Err(error)),
                };
                self.set_place(&args[1], checked, environment)
            }
            "DOCUMENTATION" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf documentation", "two", args.len())));
                }
                let object = match self.eval_in(&args[0], environment) {
                    Ok(object) => object,
                    Err(error) => return Some(Err(error)),
                };
                let doc_type = match self.eval_in(&args[1], environment) {
                    Ok(doc_type) => doc_type,
                    Err(error) => return Some(Err(error)),
                };
                let (doc_type, _) = match doc_type.symbol_reference() {
                    Some(reference) => reference,
                    None => {
                        return Some(Err(self.invalid(
                            "setf documentation type must be a symbol",
                            args[1].span,
                        )));
                    }
                };
                let documentation = match value {
                    Value::Nil => None,
                    Value::String(text) => Some(text.to_string()),
                    other => {
                        return Some(Err(RuntimeError::Type {
                            expected: "STRING or NIL".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(place.span),
                        }));
                    }
                };
                match object {
                    Value::Class(class) => {
                        *class.documentation.borrow_mut() = documentation;
                        Ok(())
                    }
                    Value::Package(package) => {
                        if self
                            .packages
                            .borrow_mut()
                            .set_package_documentation(package.as_ref(), documentation)
                        {
                            Ok(())
                        } else {
                            Err(self.package_error(
                                &format!("unknown package {}", package.as_ref()),
                                args[0].span,
                            ))
                        }
                    }
                    object
                        if matches!(
                            unqualified_name(doc_type).as_str(),
                            "FUNCTION" | "VARIABLE"
                        ) =>
                    {
                        let (name, exact) = match object.symbol_reference() {
                            Some(reference) => reference,
                            None => {
                                return Some(Err(self.invalid(
                                    "setf documentation target must be a symbol",
                                    args[0].span,
                                )));
                            }
                        };
                        match unqualified_name(doc_type).as_str() {
                            "FUNCTION" => {
                                if exact {
                                    environment
                                        .set_function_documentation_exact(name, documentation);
                                } else {
                                    environment.set_function_documentation(name, documentation);
                                }
                            }
                            "VARIABLE" => {
                                if exact {
                                    environment
                                        .set_variable_documentation_exact(name, documentation);
                                } else {
                                    environment.set_variable_documentation(name, documentation);
                                }
                            }
                            _ => unreachable!("documentation type was matched above"),
                        }
                        Ok(())
                    }
                    _ => Err(self.invalid("unsupported SETF DOCUMENTATION type", args[1].span)),
                }
            }
            _ => return None,
        })
    }
}

impl Runtime {
    fn apply_clos_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "MAKE-INSTANCE" => self.make_instance(arguments, environment, span),
            "ALLOCATE-INSTANCE" => self.allocate_instance(arguments, environment, span),
            "CHANGE-CLASS" => self.change_class(arguments, environment, span),
            "REINITIALIZE-INSTANCE" => self.reinitialize_instance(arguments, environment, span),
            "SHARED-INITIALIZE" => self.shared_initialize(arguments, environment, span),
            "ENSURE-GENERIC-FUNCTION" => self.ensure_generic_function(arguments, environment, span),
            "FIND-METHOD" => self.find_method(arguments, environment, span),
            "COMPUTE-APPLICABLE-METHODS" => {
                self.compute_applicable_methods(arguments, environment, span)
            }
            "GENERIC-FUNCTION-METHODS" => {
                self.generic_function_methods(arguments, environment, span)
            }
            "GENERIC-FUNCTION-CLASS" => self.generic_function_class(arguments, environment, span),
            "GENERIC-FUNCTION-NAME" => self.generic_function_name(arguments, environment, span),
            "METHOD-CLASS" => self.method_class(arguments, environment, span),
            "METHOD-COMBINATION" => self.method_combination(arguments, environment, span),
            "METHOD-FUNCTION" => self.method_function(arguments, span),
            "METHOD-GENERIC-FUNCTION" => self.method_generic_function(arguments, environment, span),
            "METHOD-LAMBDA-LIST" => self.method_lambda_list(arguments, span),
            "METHOD-QUALIFIERS" => self.method_qualifiers(arguments, span),
            "METHOD-SPECIALIZERS" => self.method_specializers(arguments, environment, span),
            "SLOT-VALUE" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot-value", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                let Some(class) = arguments[0].instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                let Some(value) = arguments[0].instance_slot(&slot_name) else {
                    return self.slot_missing(
                        class,
                        &arguments[0],
                        &slot_name,
                        "SLOT-VALUE",
                        None,
                        EvaluationContext { environment, span },
                    );
                };
                if matches!(value, Value::Unbound) {
                    return self.slot_unbound(class, &arguments[0], &slot_name, environment, span);
                }
                Ok(value)
            }
            "SUBTYPEP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("subtypep", "two", arguments.len()));
                }
                builtins::subtypep_value(&arguments[0], &arguments[1], environment)
            }
            "UPGRADED-ARRAY-ELEMENT-TYPE" => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Err(self.arity(
                        "upgraded-array-element-type",
                        "one or two",
                        arguments.len(),
                    ));
                }
                builtins::upgraded_array_element_type_value(&arguments[0], environment)
            }
            "CLASS-OF" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-of", "one", arguments.len()));
                }
                let class = match &arguments[0] {
                    Value::Instance(instance) => instance.class.borrow().clone(),
                    Value::Structure { name, .. } => Rc::new(ClassDefinition {
                        name: name.to_string(),
                        precedence: vec![
                            name.to_string(),
                            "STRUCTURE".to_owned(),
                            "STANDARD-OBJECT".to_owned(),
                        ],
                        slots: Vec::new(),
                        default_initargs: Vec::new(),
                        documentation: Rc::new(RefCell::new(None)),
                    }),
                    value => {
                        let name = value.type_name().to_owned();
                        Rc::new(ClassDefinition {
                            name: name.clone(),
                            precedence: vec![name, "STANDARD-OBJECT".to_owned()],
                            slots: Vec::new(),
                            default_initargs: Vec::new(),
                            documentation: Rc::new(RefCell::new(None)),
                        })
                    }
                };
                Ok(Value::class_object(class))
            }
            "FIND-CLASS" => {
                if !(1..=3).contains(&arguments.len()) {
                    return Err(self.arity("find-class", "one to three", arguments.len()));
                }
                let class_name = self.name_designator_from_value(&arguments[0], span)?;
                match environment.lookup_class(&class_name) {
                    Some(class) => Ok(Value::class_object(class)),
                    None if environment.lookup_structure(&class_name).is_some() => {
                        Ok(Self::class_object_named(&class_name, environment))
                    }
                    None if arguments.get(1).is_some_and(|errorp| !errorp.is_truthy()) => {
                        Ok(Value::Nil)
                    }
                    None => Err(self.invalid("unknown class", span)),
                }
            }
            "CLASS-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-name", "one", arguments.len()));
                }
                let Value::Class(class) = &arguments[0] else {
                    return Err(RuntimeError::Type {
                        expected: "CLASS".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                Ok(Value::symbol(class.name.clone()))
            }
            "SLOT-EXISTS-P" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot predicate", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                Ok(Value::boolean(
                    arguments[0].instance_slot_exists(&slot_name),
                ))
            }
            "SLOT-BOUNDP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot predicate", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                let Some(class) = arguments[0].instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                match arguments[0].instance_slot_is_bound(&slot_name) {
                    Some(bound) => Ok(Value::boolean(bound)),
                    None => self.slot_missing(
                        class,
                        &arguments[0],
                        &slot_name,
                        "SLOT-BOUNDP",
                        None,
                        EvaluationContext { environment, span },
                    ),
                }
            }
            "SLOT-MAKUNBOUND" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot-makunbound", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                let Some(class) = arguments[0].instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                if !arguments[0].set_instance_slot(&class.name, &slot_name, Value::Unbound) {
                    self.slot_missing(
                        class,
                        &arguments[0],
                        &slot_name,
                        "SLOT-MAKUNBOUND",
                        None,
                        EvaluationContext { environment, span },
                    )?;
                }
                Ok(arguments[0].clone())
            }
            "CALL-NEXT-METHOD" => {
                let (dispatch, method, continuation, default_arguments) = {
                    let contexts = self.method_context.borrow();
                    let Some(context) = contexts.last() else {
                        return Err(
                            self.invalid("call-next-method is only available in a method", span)
                        );
                    };
                    (
                        context.dispatch.clone(),
                        context.method.clone(),
                        context.next.clone(),
                        context.arguments.clone(),
                    )
                };
                let Some(continuation) = continuation else {
                    return self.no_next_method(
                        &dispatch,
                        &method,
                        &default_arguments,
                        span,
                        environment,
                    );
                };
                let next_arguments = if arguments.is_empty() {
                    default_arguments
                } else {
                    arguments.to_vec()
                };
                if !arguments.is_empty() {
                    let dispatch = match &continuation {
                        MethodContinuation::Chain { dispatch, .. }
                        | MethodContinuation::Core { dispatch, .. } => dispatch,
                        MethodContinuation::Default(_) => &dispatch,
                    };
                    let applicable =
                        self.ordered_applicable_methods(&dispatch.methods, &next_arguments);
                    if applicable
                        .iter()
                        .map(|method| method.id)
                        .ne(dispatch.applicable.iter().map(|method| method.id))
                    {
                        return Err(self.invalid(
                            &format!(
                                "call-next-method arguments changed the ordered applicable methods for {}",
                                dispatch.name
                            ),
                            span,
                        ));
                    }
                }
                self.invoke_continuation(continuation, &next_arguments, span, environment)
            }
            "NEXT-METHOD-P" => {
                if !arguments.is_empty() {
                    return Err(self.arity("next-method-p", "zero", arguments.len()));
                }
                let has_next = self
                    .method_context
                    .borrow()
                    .last()
                    .and_then(|context| context.next.as_ref())
                    .is_some();
                Ok(Value::boolean(has_next))
            }
            _ => unreachable!("CLOS primitive group was misclassified"),
        }
    }
}

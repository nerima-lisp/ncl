macro_rules! evaluator_objects {
    () => {
    fn make_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("make-instance", "at least one", arguments.len()));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(self.invalid("make-instance initargs require pairs", span));
        }
        let class = self.class_definition_from_value(&arguments[0], environment, span)?;

        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(1));
        for pair in arguments[1..].chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        for (initarg, init_form) in &class.default_initargs {
            if initargs.iter().any(|(name, _)| name == initarg) {
                continue;
            }
            initargs.push((initarg.clone(), self.eval_in(init_form, environment)?));
        }
        let instance = self.allocate_instance_for_class(class.clone());
        let mut initialize_arguments = Vec::with_capacity(arguments.len());
        initialize_arguments.push(instance.clone());
        initialize_arguments.extend(arguments[1..].iter().cloned());
        match environment.lookup_function("INITIALIZE-INSTANCE") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => self.apply_generic_with_default(
                    &function,
                    name,
                    methods,
                    &initialize_arguments,
                    Some(GenericDefaultAction::SharedInitialize {
                        instance,
                        class,
                        slot_names: Value::Boolean(true),
                        initargs,
                        unknown_initarg_message: "unknown make-instance initarg",
                    }),
                    EvaluationContext { environment, span },
                ),
                _ => {
                    self.shared_initialize_instance(
                        &instance,
                        &class,
                        &Value::Boolean(true),
                        &initargs,
                        EvaluationContext { environment, span },
                        "unknown make-instance initarg",
                    )?;
                    self.apply_in(
                        &Value::Function(function),
                        &initialize_arguments,
                        span,
                        environment,
                    )
                }
            },
            Some(function) => {
                self.shared_initialize_instance(
                    &instance,
                    &class,
                    &Value::Boolean(true),
                    &initargs,
                    EvaluationContext { environment, span },
                    "unknown make-instance initarg",
                )?;
                self.apply_in(&function, &initialize_arguments, span, environment)
            }
            None => {
                self.shared_initialize_instance(
                    &instance,
                    &class,
                    &Value::Boolean(true),
                    &initargs,
                    EvaluationContext { environment, span },
                    "unknown make-instance initarg",
                )?;
                Ok(instance)
            }
        }
    }

    fn allocate_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("allocate-instance", "one", arguments.len()));
        }
        let class = self.class_definition_from_value(&arguments[0], environment, span)?;
        Ok(self.allocate_instance_for_class(class))
    }

    fn shared_initialize(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() < 2 {
            return Err(self.arity("shared-initialize", "at least two", arguments.len()));
        }
        if !(arguments.len() - 2).is_multiple_of(2) {
            return Err(self.invalid("shared-initialize initargs require pairs", span));
        }
        let Some(class) = arguments[0].instance_class_definition() else {
            return Err(RuntimeError::Type {
                expected: "STANDARD-OBJECT".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(2));
        for pair in arguments[2..].chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        self.shared_initialize_instance(
            &arguments[0],
            &class,
            &arguments[1],
            &initargs,
            EvaluationContext { environment, span },
            "unknown shared-initialize initarg",
        )?;
        Ok(arguments[0].clone())
    }

    fn reinitialize_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("reinitialize-instance", "at least one", arguments.len()));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(self.invalid("reinitialize-instance initargs require pairs", span));
        }
        let Some(class) = arguments[0].instance_class_definition() else {
            return Err(RuntimeError::Type {
                expected: "STANDARD-OBJECT".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(1));
        for pair in arguments[1..].chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        match environment.lookup_function("REINITIALIZE-INSTANCE") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => self.apply_generic_with_default(
                    &function,
                    name,
                    methods,
                    arguments,
                    Some(GenericDefaultAction::SharedInitialize {
                        instance: arguments[0].clone(),
                        class,
                        slot_names: Value::Nil,
                        initargs,
                        unknown_initarg_message: "unknown reinitialize-instance initarg",
                    }),
                    EvaluationContext { environment, span },
                ),
                _ => {
                    self.shared_initialize_instance(
                        &arguments[0],
                        &class,
                        &Value::Nil,
                        &initargs,
                        EvaluationContext { environment, span },
                        "unknown reinitialize-instance initarg",
                    )?;
                    self.apply_in(&Value::Function(function), arguments, span, environment)
                }
            },
            Some(function) => {
                self.shared_initialize_instance(
                    &arguments[0],
                    &class,
                    &Value::Nil,
                    &initargs,
                    EvaluationContext { environment, span },
                    "unknown reinitialize-instance initarg",
                )?;
                self.apply_in(&function, arguments, span, environment)
            }
            None => {
                self.shared_initialize_instance(
                    &arguments[0],
                    &class,
                    &Value::Nil,
                    &initargs,
                    EvaluationContext { environment, span },
                    "unknown reinitialize-instance initarg",
                )?;
                Ok(arguments[0].clone())
            }
        }
    }

    fn change_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() < 2 {
            return Err(self.arity("change-class", "at least two", arguments.len()));
        }
        if !(arguments.len() - 2).is_multiple_of(2) {
            return Err(self.invalid("change-class initargs require pairs", span));
        }
        let Some(current_class) = arguments[0].instance_class_definition() else {
            return Err(RuntimeError::Type {
                expected: "STANDARD-OBJECT".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        let target_class = self.class_definition_from_value(&arguments[1], environment, span)?;

        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(2));
        for pair in arguments[2..].chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }

        let previous_slots = current_class
            .slots
            .iter()
            .map(|slot| {
                (
                    slot.name.clone(),
                    arguments[0]
                        .instance_slot(&slot.name)
                        .unwrap_or(Value::Unbound),
                )
            })
            .collect();
        let previous = Value::instance(current_class.clone(), previous_slots);

        let new_slots = target_class
            .slots
            .iter()
            .map(|slot| {
                let value = if slot.class_value.is_none() {
                    current_class
                        .slots
                        .iter()
                        .find(|current| current.name.eq_ignore_ascii_case(&slot.name))
                        .filter(|current| current.class_value.is_none())
                        .and_then(|_| arguments[0].instance_slot(&slot.name))
                        .unwrap_or(Value::Unbound)
                } else {
                    Value::Unbound
                };
                (slot.name.clone(), value)
            })
            .collect();
        if !arguments[0].replace_instance_layout(target_class.clone(), new_slots) {
            return Err(RuntimeError::Type {
                expected: "STANDARD-OBJECT".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }

        self.shared_initialize_instance(
            &arguments[0],
            &target_class,
            &Value::Boolean(true),
            &initargs,
            EvaluationContext { environment, span },
            "unknown change-class initarg",
        )?;

        let mut update_arguments = Vec::with_capacity(arguments.len());
        update_arguments.push(previous);
        update_arguments.push(arguments[0].clone());
        update_arguments.extend(arguments[2..].iter().cloned());
        match environment.lookup_function("UPDATE-INSTANCE-FOR-DIFFERENT-CLASS") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => {
                    self.apply_generic_with_default(
                        &function,
                        name,
                        methods,
                        &update_arguments,
                        Some(GenericDefaultAction::Value(arguments[0].clone())),
                        EvaluationContext { environment, span },
                    )?;
                }
                _ => {
                    self.apply_in(
                        &Value::Function(function),
                        &update_arguments,
                        span,
                        environment,
                    )?;
                }
            },
            Some(function) => {
                self.apply_in(&function, &update_arguments, span, environment)?;
            }
            None => {}
        }

        match environment.lookup_function("CHANGE-CLASS") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => self.apply_generic_with_default(
                    &function,
                    name,
                    methods,
                    arguments,
                    Some(GenericDefaultAction::Value(arguments[0].clone())),
                    EvaluationContext { environment, span },
                ),
                _ => self.apply_in(&Value::Function(function), arguments, span, environment),
            },
            Some(function) => self.apply_in(&function, arguments, span, environment),
            None => Ok(arguments[0].clone()),
        }
    }

    fn slot_missing(
        &self,
        class: Rc<ClassDefinition>,
        object: &Value,
        slot_name: &str,
        operation: &str,
        new_value: Option<Value>,
        context: EvaluationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let EvaluationContext { environment, span } = context;
        let mut arguments = vec![
            Value::class_object(class),
            object.clone(),
            Value::symbol(slot_name),
            Value::symbol(operation),
        ];
        if let Some(value) = new_value {
            arguments.push(value);
        }
        match environment.lookup_function("SLOT-MISSING") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => self.apply_generic_with_default(
                    &function,
                    name,
                    methods,
                    &arguments,
                    None,
                    EvaluationContext { environment, span },
                ),
                _ => self.apply_in(&Value::Function(function), &arguments, span, environment),
            },
            Some(function) => self.apply_in(&function, &arguments, span, environment),
            None => Err(self.invalid("slot is not defined for this class", span)),
        }
    }

    fn slot_unbound(
        &self,
        class: Rc<ClassDefinition>,
        object: &Value,
        slot_name: &str,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let arguments = vec![
            Value::class_object(class),
            object.clone(),
            Value::symbol(slot_name),
        ];
        match environment.lookup_function("SLOT-UNBOUND") {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic { name, methods, .. } => self.apply_generic_with_default(
                    &function,
                    name,
                    methods,
                    &arguments,
                    None,
                    EvaluationContext { environment, span },
                ),
                _ => self.apply_in(&Value::Function(function), &arguments, span, environment),
            },
            Some(function) => self.apply_in(&function, &arguments, span, environment),
            None => Err(self.invalid("slot is unbound", span)),
        }
    }

    fn class_definition_from_value(
        &self,
        value: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Rc<ClassDefinition>, RuntimeError> {
        match value {
            Value::Class(definition) => Ok(definition.clone()),
            _ => {
                let class_name = self.name_designator_from_value(value, span)?;
                environment
                    .lookup_class(&class_name)
                    .ok_or_else(|| self.invalid("unknown class", span))
            }
        }
    }

    fn allocate_instance_for_class(&self, class: Rc<ClassDefinition>) -> Value {
        let slots = class
            .slots
            .iter()
            .map(|slot| (slot.name.clone(), Value::Unbound))
            .collect();
        Value::instance(class, slots)
    }

    fn shared_initialize_instance(
        &self,
        instance: &Value,
        class: &Rc<ClassDefinition>,
        slot_names: &Value,
        initargs: &[(String, Value)],
        context: EvaluationContext<'_>,
        unknown_initarg_message: &str,
    ) -> Result<(), RuntimeError> {
        let EvaluationContext { environment, span } = context;
        let allow_other_keys = initargs
            .iter()
            .any(|(initarg, value)| initarg == "ALLOW-OTHER-KEYS" && value.is_truthy());
        if !allow_other_keys {
            for (initarg, _) in initargs {
                if initarg == "ALLOW-OTHER-KEYS" {
                    continue;
                }
                if !class
                    .slots
                    .iter()
                    .any(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
                {
                    return Err(self.invalid(unknown_initarg_message, span));
                }
            }
        }

        let requested_slots = if matches!(slot_names, Value::Boolean(true)) {
            None
        } else if matches!(slot_names, Value::Nil | Value::Boolean(false)) {
            Some(Vec::new())
        } else {
            let items = slot_names.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_owned(),
                actual: slot_names.type_name().to_owned(),
                span: Some(span),
            })?;
            let mut names = Vec::with_capacity(items.len());
            for item in items {
                let slot_name = self.slot_name_from_value(&item, span)?;
                if !class
                    .slots
                    .iter()
                    .any(|slot| slot.name.eq_ignore_ascii_case(&slot_name))
                {
                    return Err(self.invalid("slot is not defined for this class", span));
                }
                names.push(slot_name);
            }
            Some(names)
        };

        for slot in &class.slots {
            if let Some(initarg) = slot.initarg.as_ref()
                && let Some((_, value)) = initargs.iter().rev().find(|(name, _)| name == initarg)
            {
                if !instance.set_instance_slot(&class.name, &slot.name, value.clone()) {
                    return Err(self.invalid("slot is not defined for this class", span));
                }
                continue;
            }

            let should_initialize = match &requested_slots {
                None => true,
                Some(names) => names
                    .iter()
                    .any(|name| slot.name.eq_ignore_ascii_case(name)),
            };
            if !should_initialize || instance.instance_slot_is_bound(&slot.name) == Some(true) {
                continue;
            }

            let value = if let Some(class_value) = &slot.class_value {
                let current = class_value.borrow().clone();
                if matches!(current, Value::Unbound) {
                    let value = slot
                        .init_form
                        .as_ref()
                        .map(|form| self.eval_in(form, environment))
                        .transpose()?
                        .unwrap_or(Value::Unbound);
                    *class_value.borrow_mut() = value.clone();
                    value
                } else {
                    current
                }
            } else {
                slot.init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, environment))
                    .transpose()?
                    .unwrap_or(Value::Unbound)
            };
            if !instance.set_instance_slot(&class.name, &slot.name, value) {
                return Err(self.invalid("slot is not defined for this class", span));
            }
        }

        Ok(())
    }

    fn compile_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(1..=2).contains(&arguments.len()) {
            return Err(self.arity("compile", "one or two", arguments.len()));
        }

        let name = match &arguments[0] {
            Value::Nil | Value::Boolean(false) => None,
            value => {
                let (name, exact) = value
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("compile name must be a symbol or NIL", span))?;
                Some((name.to_owned(), exact))
            }
        };

        let function = match arguments.get(1) {
            None | Some(Value::Nil) | Some(Value::Boolean(false)) => {
                let Some((name, exact)) = name.as_ref() else {
                    return Err(self.invalid(
                        "compile needs a function definition when the name is NIL",
                        span,
                    ));
                };
                let function = if *exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match function {
                    Some(value @ Value::Function(_)) => value,
                    Some(value) => {
                        return Err(RuntimeError::NotCallable {
                            value: value.to_string(),
                            span: Some(span),
                        });
                    }
                    None => {
                        return Err(RuntimeError::UnboundVariable {
                            name: name.clone(),
                            span: Some(span),
                        });
                    }
                }
            }
            Some(definition) => {
                let form = self.form_from_value(definition, span)?;
                let expanded = self.prepare_compiled_form(&form, environment)?;
                let program = Rc::new(Compiler::compile_form(&expanded)?);
                crate::vm::run_entry(self, program, 0, environment.clone(), expanded.span)?
                    .primary_value()
            }
        };

        if !matches!(function, Value::Function(_)) {
            return Err(RuntimeError::Type {
                expected: "FUNCTION".to_owned(),
                actual: function.type_name().to_owned(),
                span: Some(span),
            });
        }

        if let Some((name, exact)) = name {
            if exact {
                environment.define_function_exact(name, function.clone());
            } else {
                environment.define_function(name, function.clone());
            }
        }

        Ok(Value::values(vec![function, Value::Nil, Value::Nil]))
    }


    };
}

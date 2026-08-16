macro_rules! evaluator_primitives {
    () => {
    fn apply_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "ERROR" => {
                if arguments.is_empty() {
                    return Err(self.arity("error", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    let error = Self::condition_error(&arguments[0], false, span)?;
                    return match self.dispatch_condition(
                        error.clone(),
                        &arguments[0],
                        environment,
                        span,
                    ) {
                        Ok(()) => Err(error),
                        Err(error) => Err(error),
                    };
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                let message = Self::condition_message(&arguments[0], format_arguments, span)?;
                let error = Self::signaled_error(
                    "SIMPLE-ERROR",
                    Vec::new(),
                    message.clone(),
                    format_control.clone(),
                    format_arguments,
                    false,
                    span,
                );
                match self.signal_condition(
                    "SIMPLE-ERROR",
                    message.clone(),
                    format_control,
                    format_arguments,
                    false,
                    EvaluationContext { environment, span },
                ) {
                    Ok(()) => Err(error),
                    Err(error) => Err(error),
                }
            }
            "SIGNAL" => {
                if arguments.is_empty() {
                    return Err(self.arity("signal", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    if arguments.len() != 1 {
                        return Err(self.invalid(
                            "signal does not accept format arguments with a condition object",
                            span,
                        ));
                    }
                    self.signal_condition_value(&arguments[0], false, environment, span)?;
                    return Ok(Value::Nil);
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                self.signal_condition(
                    "SIMPLE-CONDITION",
                    Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    false,
                    EvaluationContext { environment, span },
                )?;
                Ok(Value::Nil)
            }
            "WARN" => {
                if arguments.is_empty() {
                    return Err(self.arity("warn", "at least one", arguments.len()));
                }
                if arguments[0].condition_type_name().is_some() {
                    if arguments.len() != 1 {
                        return Err(self.invalid(
                            "warn does not accept format arguments with a condition object",
                            span,
                        ));
                    }
                    self.signal_condition_value(&arguments[0], true, environment, span)?;
                    return Ok(Value::Nil);
                }
                let format_arguments = &arguments[1..];
                let format_control = Self::condition_format_control(&arguments[0]);
                self.signal_condition(
                    "SIMPLE-WARNING",
                    Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    true,
                    EvaluationContext { environment, span },
                )?;
                Ok(Value::Nil)
            }
            "CERROR" => {
                if arguments.len() < 2 {
                    return Err(self.arity("cerror", "at least two", arguments.len()));
                }
                let format_arguments = &arguments[2..];
                let _continue_message =
                    Self::condition_message(&arguments[0], format_arguments, span)?;
                let condition_object = arguments[1].condition_type_name().is_some();
                if condition_object && !format_arguments.is_empty() {
                    return Err(self.invalid(
                        "cerror does not accept format arguments with a condition object",
                        span,
                    ));
                }
                let format_control = Self::condition_format_control(&arguments[1]);
                let message = Self::condition_message(&arguments[1], format_arguments, span)?;
                let signal_result = if condition_object {
                    let error = Self::condition_error(&arguments[1], false, span)?;
                    self.dispatch_condition(error, &arguments[1], environment, span)
                } else {
                    self.signal_condition(
                        "SIMPLE-ERROR",
                        message.clone(),
                        format_control,
                        format_arguments,
                        false,
                        EvaluationContext { environment, span },
                    )
                };
                match signal_result {
                    Ok(()) => {}
                    Err(error @ RuntimeError::InvokeRestart { .. }) => {
                        let RuntimeError::InvokeRestart { name, .. } = &error else {
                            unreachable!()
                        };
                        if normalize_name(name) == "CONTINUE" {
                            return Ok(Value::Nil);
                        }
                        return Err(error);
                    }
                    Err(error) => return Err(error),
                }
                if self
                    .restart_bindings()
                    .iter()
                    .any(|binding| normalize_name(&binding.name) == "CONTINUE")
                {
                    self.invoke_restart_named("CONTINUE", &[], environment, span)
                } else {
                    Err(RuntimeError::InvalidForm {
                        message,
                        span: Some(span),
                    })
                }
            }
            "MAKE-CONDITION" => self.make_condition(arguments, environment, span),
            "EVAL" => {
                if arguments.len() != 1 {
                    return Err(self.arity("eval", "one", arguments.len()));
                }
                let form = self.form_from_value(&arguments[0], span)?;
                self.eval_values_in(&form, environment)
            }
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
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
            "MAKE-SYMBOL" => {
                if arguments.len() != 1 {
                    return Err(self.arity("make-symbol", "one", arguments.len()));
                }
                let Some(Value::String(name)) = arguments.first() else {
                    return Err(self.invalid("make-symbol argument must be a string", span));
                };
                Ok(Value::uninterned_symbol(name.as_ref()))
            }
            "GENSYM" => {
                if arguments.len() > 1 {
                    return Err(self.arity("gensym", "zero or one", arguments.len()));
                }
                let prefix = match arguments.first() {
                    None => "G".to_string(),
                    Some(Value::String(value)) => value.to_string(),
                    Some(value) => value
                        .symbol_name()
                        .map(|name| name.to_string())
                        .ok_or_else(|| {
                            self.invalid("gensym prefix must be a string designator", span)
                        })?,
                };
                let counter = self.gensym_counter.get();
                self.gensym_counter.set(counter.wrapping_add(1));
                Ok(Value::uninterned_symbol(format!("{prefix}{counter}")))
            }
            "MAKE-PACKAGE" => self.make_package_from_arguments(arguments, span),
            "INTERN" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("intern", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = match self
                    .packages
                    .borrow_mut()
                    .intern_symbol(&package_name, &symbol_name)
                {
                    Some(status) => status,
                    None => {
                        return Err(
                            self.package_error(&format!("unknown package {package_name}"), span)
                        );
                    }
                };
                let symbol = self.package_symbol_value(&package_name, &symbol_name);
                Ok(Value::values(vec![
                    symbol,
                    Self::symbol_status_value(status),
                ]))
            }
            "FIND-SYMBOL" => {
                if !(1..=2).contains(&arguments.len()) {
                    return Err(self.arity("find-symbol", "one or two", arguments.len()));
                }
                let symbol_name = self.symbol_name_from_value(&arguments[0], span)?;
                let package_name = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let status = self
                    .packages
                    .borrow()
                    .symbol_status(&package_name, &symbol_name);
                match status {
                    Some(status) => {
                        let symbol = self.package_symbol_value(&package_name, &symbol_name);
                        Ok(Value::values(vec![
                            symbol,
                            Self::symbol_status_value(status),
                        ]))
                    }
                    None => Ok(Value::values(vec![Value::Nil, Value::Nil])),
                }
            }
            "FIND-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-package", "one", arguments.len()));
                }
                let package = self.package_designator_name(&arguments[0], span)?;
                let packages = self.packages.borrow();
                if packages.package_exists(&package) {
                    Ok(Value::package(packages.canonical_package_name(&package)))
                } else {
                    Ok(Value::Nil)
                }
            }
            "DELETE-PACKAGE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("delete-package", "one", arguments.len()));
                }
                let package = self.package_name_from_value(&arguments[0], span)?;
                let deleted = self
                    .packages
                    .borrow_mut()
                    .delete_package(&package)
                    .map_err(|message| self.package_error(&message, span))?;
                Ok(Value::boolean(deleted))
            }
            "RENAME-PACKAGE" => {
                if !(2..=3).contains(&arguments.len()) {
                    return Err(self.arity("rename-package", "two or three", arguments.len()));
                }
                let package = self.package_name_from_value(&arguments[0], span)?;
                let new_name = self.name_designator_from_value(&arguments[1], span)?;
                let nicknames = arguments
                    .get(2)
                    .map(|value| self.package_nicknames_from_value(value, span))
                    .transpose()?
                    .unwrap_or_default();
                let name = self
                    .packages
                    .borrow_mut()
                    .rename_package(&package, new_name, nicknames)
                    .map_err(|message| self.package_error(&message, span))?;
                Ok(Value::package(name))
            }
            "PACKAGE-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-name", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => Ok(Value::string(package.as_ref())),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USE-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-use-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let names = self.packages.borrow().use_packages_for(package);
                        Ok(Value::list(names.into_iter().map(Value::package).collect()))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-NICKNAMES" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-nicknames", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let nicknames = self.packages.borrow().package_nicknames(package);
                        Ok(Value::list(
                            nicknames.into_iter().map(Value::string).collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-SHADOWING-SYMBOLS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-shadowing-symbols", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let symbols = self
                            .packages
                            .borrow()
                            .shadowing_symbols_for(package)
                            .into_iter()
                            .map(|symbol| {
                                self.package_symbol_value(symbol.package(), symbol.name())
                            })
                            .collect();
                        Ok(Value::list(symbols))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "PACKAGE-USED-BY-LIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("package-used-by-list", "one", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => {
                        let packages = self.packages.borrow().used_by_packages_for(package);
                        Ok(Value::list(
                            packages.into_iter().map(Value::package).collect(),
                        ))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "DOCUMENTATION" => {
                if arguments.len() != 2 {
                    return Err(self.arity("documentation", "two", arguments.len()));
                }
                match &arguments[0] {
                    Value::Class(class) => {
                        let documentation = class.documentation.borrow();
                        Ok(documentation.as_ref().map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        }))
                    }
                    Value::Package(package) => Ok(self
                        .packages
                        .borrow()
                        .package_documentation(package)
                        .map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        })),
                    other if other.symbol_reference().is_some() => {
                        let (name, exact) = other.symbol_reference().expect("symbol reference");
                        let (doc_type, _) = arguments[1].symbol_reference().ok_or_else(|| {
                            self.invalid("documentation type must be a symbol", span)
                        })?;
                        let documentation = match unqualified_name(doc_type).as_str() {
                            "FUNCTION" => {
                                if exact {
                                    environment.lookup_function_documentation_exact(name)
                                } else {
                                    environment.lookup_function_documentation(name)
                                }
                            }
                            "VARIABLE" => {
                                if exact {
                                    environment.lookup_variable_documentation_exact(name)
                                } else {
                                    environment.lookup_variable_documentation(name)
                                }
                            }
                            _ => None,
                        };
                        Ok(documentation.map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        }))
                    }
                    other => Err(RuntimeError::Type {
                        expected: "CLASS, PACKAGE, or SYMBOL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                }
            }
            "LIST-ALL-PACKAGES" => {
                if !arguments.is_empty() {
                    return Err(self.arity("list-all-packages", "zero", arguments.len()));
                }
                let names = self.packages.borrow().all_package_names();
                Ok(Value::list(names.into_iter().map(Value::package).collect()))
            }
            "USE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("use-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                if packages.iter().any(|package| package == &target) {
                    return Err(self.package_error("a package cannot use itself", span));
                }
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state
                        .use_package(&package, &target)
                        .map_err(|message| self.package_error(&message, span))?;
                }
                Ok(Value::boolean(true))
            }
            "UNUSE-PACKAGE" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unuse-package", "one or two", arguments.len()));
                }
                let packages = self.package_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for package in packages {
                    state.unuse_package(&package, &target);
                }
                Ok(Value::boolean(true))
            }
            "EXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("export", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .export_symbols(&package, &symbols);
                Ok(Value::boolean(true))
            }
            "UNEXPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unexport", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let package = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                self.packages
                    .borrow_mut()
                    .unexport_symbols(&package, &symbols);
                Ok(Value::boolean(true))
            }
            "IMPORT" | "SHADOWING-IMPORT" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity(name, "one or two", arguments.len()));
                }
                let imports = self.symbol_import_references_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                {
                    let state = self.packages.borrow();
                    for (source_package, source_name) in &imports {
                        if !state.symbol_exists(source_package, source_name) {
                            return Err(self.package_error(
                                &format!("unknown symbol {source_package}::{source_name}"),
                                span,
                            ));
                        }
                    }
                }
                let shadowing = name == "SHADOWING-IMPORT";
                let mut state = self.packages.borrow_mut();
                for (source_package, source_name) in imports {
                    state.import_symbol(&source_package, &source_name, &target, shadowing);
                }
                Ok(Value::boolean(true))
            }
            "SHADOW" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("shadow", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut state = self.packages.borrow_mut();
                for symbol in symbols {
                    state.shadow_symbol(&target, &symbol);
                }
                Ok(Value::boolean(true))
            }
            "UNINTERN" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("unintern", "one or two", arguments.len()));
                }
                let symbols = self.symbol_names_from_value(&arguments[0], span)?;
                let target = arguments
                    .get(1)
                    .map(|value| self.package_name_from_value(value, span))
                    .transpose()?
                    .unwrap_or_else(|| self.current_package());
                let mut removed = false;
                let mut local_names = Vec::new();
                {
                    let mut state = self.packages.borrow_mut();
                    for symbol in symbols {
                        let local_name = package::canonical_symbol_name(&target, &symbol);
                        removed |= state.unintern_symbol(&target, &symbol);
                        local_names.push(local_name);
                    }
                }
                for local_name in local_names {
                    self.remove_global_symbol(&local_name);
                }
                Ok(Value::boolean(removed))
            }
            "BOUNDP" => {
                if arguments.len() != 1 {
                    return Err(self.arity("boundp", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("boundp argument must be a symbol", span))?;
                Ok(Value::boolean(if exact {
                    self.is_bound_exact_in(name, environment)
                } else {
                    self.is_bound_in(name, environment)
                }))
            }
            "CONSTANTP" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("constantp", "one or two", arguments.len()));
                }
                let environment = match arguments.get(1) {
                    None | Some(Value::Nil) => None,
                    Some(Value::Environment(environment)) => Some(environment),
                    Some(_) => {
                        return Err(
                            self.invalid("constantp environment must be an environment", span)
                        );
                    }
                };
                Ok(Value::boolean(
                    self.constantp_in(&arguments[0], environment),
                ))
            }
            "FBOUNDP" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fboundp", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fboundp argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                Ok(Value::boolean(matches!(value, Some(Value::Function(_)))))
            }
            "MACRO-FUNCTION" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity("macro-function", "one or two", arguments.len()));
                }
                let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("macro-function argument must be a symbol", span)
                })?;
                let lookup_environment = match arguments.get(1) {
                    None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
                    Some(Value::Environment(environment)) => environment,
                    Some(_) => {
                        return Err(
                            self.invalid("macro-function environment must be an environment", span)
                        );
                    }
                };
                let value = if exact {
                    self.lookup_function_exact_in(name, lookup_environment)
                } else {
                    self.lookup_function_in(name, lookup_environment)
                };
                Ok(match value {
                    Some(Value::Function(function))
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        Value::Function(function)
                    }
                    _ => Value::Nil,
                })
            }
            "COMPILER-MACRO-FUNCTION" => {
                if arguments.len() != 1 && arguments.len() != 2 {
                    return Err(self.arity(
                        "compiler-macro-function",
                        "one or two",
                        arguments.len(),
                    ));
                }
                let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("compiler-macro-function argument must be a symbol", span)
                })?;
                let lookup_environment = match arguments.get(1) {
                    None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
                    Some(Value::Environment(environment)) => environment,
                    Some(_) => {
                        return Err(self.invalid(
                            "compiler-macro-function environment must be an environment",
                            span,
                        ));
                    }
                };
                let value = if exact {
                    lookup_environment.lookup_compiler_macro_exact(name)
                } else {
                    lookup_environment.lookup_compiler_macro(name)
                };
                Ok(match value {
                    Some(Value::Function(function))
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        Value::Function(function)
                    }
                    _ => Value::Nil,
                })
            }
            "SPECIAL-OPERATOR-P" => {
                if arguments.len() != 1 {
                    return Err(self.arity("special-operator-p", "one", arguments.len()));
                }
                let (name, _) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("special-operator-p argument must be a symbol", span)
                })?;
                Ok(Value::boolean(is_special_operator_name(name)))
            }
            "COMPILED-FUNCTION-P" => {
                if arguments.len() != 1 {
                    return Err(self.arity("compiled-function-p", "one", arguments.len()));
                }
                Ok(Value::boolean(matches!(
                    &arguments[0],
                    Value::Function(function)
                        if matches!(function.as_ref(), crate::Function::Compiled { .. })
                )))
            }
            "FUNCTION-LAMBDA-EXPRESSION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("function-lambda-expression", "one", arguments.len()));
                }
                let Value::Function(function) = &arguments[0] else {
                    return Err(self.invalid(
                        "function-lambda-expression argument must be a function",
                        span,
                    ));
                };
                match function.as_ref() {
                    crate::Function::Closure {
                        parameters,
                        required_escaped,
                        optional,
                        rest,
                        rest_escaped,
                        keywords,
                        has_keyword_section,
                        allow_other_keys,
                        auxiliary,
                        body,
                        ..
                    } => Ok(Value::values(vec![
                        quoted_form_value(&closure_lambda_form(ClosureLambdaForm {
                            parameters,
                            required_escaped,
                            optional,
                            rest,
                            rest_escaped: *rest_escaped,
                            keywords,
                            has_keyword_section: *has_keyword_section,
                            allow_other_keys: *allow_other_keys,
                            auxiliary,
                            body,
                        }))?,
                        Value::boolean(true),
                        Value::Nil,
                    ])),
                    _ => Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil])),
                }
            }
            "FDEFINITION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fdefinition", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fdefinition argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match value {
                    Some(Value::Function(function)) => Ok(Value::Function(function)),
                    Some(value) => Err(RuntimeError::NotCallable {
                        value: value.to_string(),
                        span: Some(span),
                    }),
                    None => Err(RuntimeError::UnboundVariable {
                        name: if exact {
                            name.to_string()
                        } else {
                            normalize_name(name)
                        },
                        span: Some(span),
                    }),
                }
            }
            "SYMBOL-FUNCTION" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-function", "one", arguments.len()));
                }
                let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                    self.invalid("symbol-function argument must be a symbol", span)
                })?;
                let value = if exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match value {
                    Some(Value::Function(function)) => Ok(Value::Function(function)),
                    Some(value) => Err(RuntimeError::NotCallable {
                        value: value.to_string(),
                        span: Some(span),
                    }),
                    None => Err(RuntimeError::UnboundVariable {
                        name: if exact {
                            name.to_string()
                        } else {
                            normalize_name(name)
                        },
                        span: Some(span),
                    }),
                }
            }
            "SYMBOL-VALUE" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-value", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("symbol-value argument must be a symbol", span))?;
                let value = if exact {
                    self.lookup_exact_in(name, environment)
                } else {
                    self.lookup_in(name, environment)
                };
                value.ok_or_else(|| RuntimeError::UnboundVariable {
                    name: if exact {
                        name.to_string()
                    } else {
                        normalize_name(name)
                    },
                    span: Some(span),
                })
            }
            "GET" => {
                if !(2..=3).contains(&arguments.len()) {
                    return Err(self.arity("get", "two or three", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("get first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("GET needs an even property list", span));
                }
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&arguments[1]) {
                        return Ok(properties[index + 1].clone());
                    }
                }
                Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
            }
            "PUTPROP" => {
                if arguments.len() != 3 {
                    return Err(self.arity("putprop", "three", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("putprop first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("PUTPROP needs an even property list", span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&arguments[2]) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = arguments[1].clone();
                } else {
                    properties.push(arguments[2].clone());
                    properties.push(arguments[1].clone());
                }
                environment.set_symbol_plist(&arguments[0], Value::list(properties));
                Ok(arguments[1].clone())
            }
            "REMPROP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("remprop", "two", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("remprop first argument must be a symbol", span));
                }
                let plist = environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("REMPROP needs an even property list", span));
                }
                let Some(index) = (0..properties.len())
                    .step_by(2)
                    .find(|index| properties[*index].eq_value(&arguments[1]))
                else {
                    return Ok(Value::Nil);
                };
                properties.drain(index..index + 2);
                if properties.is_empty() {
                    environment.remove_symbol_property(&arguments[0]);
                } else {
                    environment.set_symbol_plist(&arguments[0], Value::list(properties));
                }
                Ok(Value::boolean(true))
            }
            "SYMBOL-PLIST" => {
                if arguments.len() != 1 {
                    return Err(self.arity("symbol-plist", "one", arguments.len()));
                }
                if arguments[0].symbol_reference().is_none() {
                    return Err(self.invalid("symbol-plist argument must be a symbol", span));
                }
                Ok(environment
                    .symbol_plist(&arguments[0])
                    .unwrap_or(Value::Nil))
            }
            "SET" => {
                if arguments.len() != 2 {
                    return Err(self.arity("set", "two", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("set first argument must be a symbol", span))?;
                self.ensure_symbol_writable(name, exact, span)?;
                Ok(if exact {
                    self.set_symbol_value_exact(name, arguments[1].clone())
                } else {
                    self.set_symbol_value(name, arguments[1].clone())
                })
            }
            "MAKUNBOUND" => {
                if arguments.len() != 1 {
                    return Err(self.arity("makunbound", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("makunbound argument must be a symbol", span))?;
                self.ensure_symbol_writable(name, exact, span)?;
                if exact {
                    self.makunbound_exact_symbol(name);
                } else {
                    self.makunbound_symbol(name);
                }
                Ok(arguments[0].clone())
            }
            "FMAKUNBOUND" => {
                if arguments.len() != 1 {
                    return Err(self.arity("fmakunbound", "one", arguments.len()));
                }
                let (name, exact) = arguments[0]
                    .symbol_reference()
                    .ok_or_else(|| self.invalid("fmakunbound argument must be a symbol", span))?;
                if exact {
                    self.fmakunbound_exact_symbol(name);
                } else {
                    self.fmakunbound_symbol(name);
                }
                Ok(arguments[0].clone())
            }
            "COMPUTE-RESTARTS" => {
                if arguments.len() > 1 {
                    return Err(self.arity("compute-restarts", "at most one", arguments.len()));
                }
                let condition = arguments
                    .first()
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    });
                }
                Ok(Value::list(
                    self.restart_bindings_for_condition(condition)
                        .into_iter()
                        .rev()
                        .map(|binding| binding.restart)
                        .collect(),
                ))
            }
            "FIND-RESTART" => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Err(self.arity("find-restart", "one or two", arguments.len()));
                }
                let condition = arguments
                    .get(1)
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    });
                }
                let bindings = self.restart_bindings_for_condition(condition);
                Ok(self
                    .restart_binding_for_designator_in(&arguments[0], &bindings, span)?
                    .map(|binding| binding.restart)
                    .unwrap_or(Value::Nil))
            }
            "RESTART-NAME" => {
                if arguments.len() != 1 {
                    return Err(self.arity("restart-name", "one", arguments.len()));
                }
                let Some(name) = arguments[0].restart_name() else {
                    return Err(RuntimeError::Type {
                        expected: "RESTART".to_string(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                };
                Ok(Value::symbol(name))
            }
            "INVOKE-RESTART" => {
                if arguments.is_empty() {
                    return Err(self.arity("invoke-restart", "at least one", arguments.len()));
                }
                if let Some((name, _)) = arguments[0].symbol_reference() {
                    return self.invoke_restart_named(name, &arguments[1..], environment, span);
                }
                let Some(binding) = self.restart_binding_for_designator(&arguments[0], span)?
                else {
                    return Err(self.invalid("restart is not active", span));
                };
                self.invoke_restart_binding(binding, &arguments[1..], environment, span)
            }
            "MAP" => {
                if arguments.len() < 3 {
                    return Err(self.arity("map", "at least three", arguments.len()));
                }
                self.apply_sequence_mapping(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REDUCE" => {
                if arguments.len() < 2 {
                    return Err(self.arity("reduce", "at least two", arguments.len()));
                }
                self.apply_sequence_reduce(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF" | "DELETE-IF-NOT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_remove(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" => {
                if arguments.is_empty() {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least one", arguments.len()));
                }
                self.apply_sequence_remove(
                    name,
                    &Value::Nil,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                )
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => {
                if arguments.len() < 3 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least three", arguments.len()));
                }
                self.apply_sequence_substitute(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2],
                    &arguments[3..],
                    EvaluationContext { environment, span },
                )
            }
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_set_operation(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_membership(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_association_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "FIND" | "POSITION" | "COUNT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "SEARCH" | "MISMATCH" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_pair_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "SORT" | "STABLE-SORT" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_sort(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MERGE" => {
                if arguments.len() < 4 {
                    return Err(self.arity("merge", "at least four", arguments.len()));
                }
                self.apply_sequence_merge(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2],
                    &arguments[3],
                    &arguments[4..],
                    EvaluationContext { environment, span },
                )
            }
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_sequence_quantifier(
                    name,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                )
            }
            "MAP-INTO" => {
                if arguments.len() < 2 {
                    return Err(self.arity("map-into", "at least two", arguments.len()));
                }
                self.apply_sequence_map_into(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON" => {
                if arguments.len() < 2 {
                    let function_name = name.to_ascii_lowercase();
                    return Err(self.arity(&function_name, "at least two", arguments.len()));
                }
                self.apply_list_mapping(name, &arguments[0], &arguments[1..], environment, span)
            }
            _ => Err(self.invalid("unknown runtime primitive", span)),
        }
    }


    };
}

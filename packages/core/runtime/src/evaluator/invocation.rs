use super::*;

impl Runtime {
    pub(super) fn make_instance(
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
        let class_name = self.name_designator_from_value(&arguments[0], span)?;
        let class = environment
            .lookup_class(&class_name)
            .ok_or_else(|| self.invalid("unknown class", span))?;

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
        for (initarg, _) in &initargs {
            if !class
                .slots
                .iter()
                .any(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            {
                return Err(self.invalid("unknown make-instance initarg", span));
            }
        }

        let mut slots = Vec::with_capacity(class.slots.len());
        for slot in &class.slots {
            let initarg_value = slot.initarg.as_ref().and_then(|initarg| {
                initargs
                    .iter()
                    .rev()
                    .find(|(name, _)| name == initarg)
                    .map(|(_, value)| value.clone())
            });
            let value = if let Some(initarg_value) = initarg_value {
                initarg_value
            } else if let Some(class_value) = &slot.class_value {
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
            slots.push((slot.name.clone(), value));
        }
        let instance = Value::instance(class.clone(), slots);
        for (initarg, value) in initargs {
            let Some(index) = class
                .slots
                .iter()
                .position(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            else {
                return Err(self.invalid("unknown make-instance initarg", span));
            };
            if !instance.set_instance_slot(&class.name, &class.slots[index].name, value) {
                return Err(self.invalid("unknown make-instance initarg", span));
            }
        }
        Ok(instance)
    }

    pub(super) fn compile_function(
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

    pub(super) fn load_file(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("load", "one", arguments.len()));
        }
        let path = match &arguments[0] {
            Value::String(path) => path.to_string(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "PATHNAME-DESIGNATOR".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let source = fs::read_to_string(&path)
            .map_err(|error| RuntimeError::Io(format!("cannot load {}: {}", path, error)))?;
        self.eval_source(&source)?;
        Ok(Value::boolean(true))
    }

    pub(super) fn apply_primitive(
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
                match self.signal_condition(SignalRequest {
                    condition: "SIMPLE-ERROR",
                    message: message.clone(),
                    format_control,
                    format_arguments,
                    warning: false,
                    environment,
                    span,
                }) {
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
                self.signal_condition(SignalRequest {
                    condition: "SIMPLE-CONDITION",
                    message: Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    warning: false,
                    environment,
                    span,
                })?;
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
                self.signal_condition(SignalRequest {
                    condition: "SIMPLE-WARNING",
                    message: Self::condition_message(&arguments[0], format_arguments, span)?,
                    format_control,
                    format_arguments,
                    warning: true,
                    environment,
                    span,
                })?;
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
                    self.signal_condition(SignalRequest {
                        condition: "SIMPLE-ERROR",
                        message: message.clone(),
                        format_control,
                        format_arguments,
                        warning: false,
                        environment,
                        span,
                    })
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
            "MAKE-CONDITION" => self.make_condition(arguments, span),
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
            "SLOT-VALUE" => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot-value", "two", arguments.len()));
                }
                let slot_name = self.slot_name_from_value(&arguments[1], span)?;
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                let value = arguments[0]
                    .instance_slot(&slot_name)
                    .ok_or_else(|| self.invalid("slot is not defined for this class", span))?;
                if matches!(value, Value::Unbound) {
                    return Err(self.invalid("slot is unbound", span));
                }
                Ok(value)
            }
            "SUBTYPEP" => {
                if arguments.len() != 2 {
                    return Err(self.arity("subtypep", "two", arguments.len()));
                }
                builtins::subtypep_value(&arguments[0], &arguments[1], environment)
            }
            "CLASS-OF" => {
                if arguments.len() != 1 {
                    return Err(self.arity("class-of", "one", arguments.len()));
                }
                let class = match &arguments[0] {
                    Value::Instance(instance) => instance.class.clone(),
                    value => {
                        let name = value.type_name().to_owned();
                        Rc::new(ClassDefinition {
                            name: name.clone(),
                            precedence: vec![name, "STANDARD-OBJECT".to_owned()],
                            slots: Vec::new(),
                            default_initargs: Vec::new(),
                        })
                    }
                };
                Ok(Value::class_object(class))
            }
            "FIND-CLASS" => {
                if arguments.len() != 1 {
                    return Err(self.arity("find-class", "one", arguments.len()));
                }
                let class_name = self.name_designator_from_value(&arguments[0], span)?;
                let class = environment
                    .lookup_class(&class_name)
                    .ok_or_else(|| self.invalid("unknown class", span))?;
                Ok(Value::class_object(class))
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
                if !matches!(arguments[0], Value::Instance(_)) {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_owned(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                Ok(Value::boolean(
                    arguments[0]
                        .instance_slot_is_bound(&slot_name)
                        .unwrap_or(false),
                ))
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
                if !arguments[0].instance_slot_exists(&slot_name)
                    || !arguments[0].set_instance_slot(&class.name, &slot_name, Value::Unbound)
                {
                    return Err(self.invalid("slot is not defined for this class", span));
                }
                Ok(arguments[0].clone())
            }
            "CALL-NEXT-METHOD" => {
                let (continuation, default_arguments) = {
                    let contexts = self.method_context.borrow();
                    let Some(context) = contexts.last() else {
                        return Err(
                            self.invalid("call-next-method is only available in a method", span)
                        );
                    };
                    (context.next.clone(), context.arguments.clone())
                };
                let Some(continuation) = continuation else {
                    return Err(self.invalid("no next method is applicable", span));
                };
                let next_arguments = if arguments.is_empty() {
                    default_arguments
                } else {
                    arguments.to_vec()
                };
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
            "DOCUMENTATION" => {
                if arguments.len() != 2 {
                    return Err(self.arity("documentation", "two", arguments.len()));
                }
                match &arguments[0] {
                    Value::Package(package) => Ok(self
                        .packages
                        .borrow()
                        .package_documentation(package)
                        .map_or(Value::Nil, |documentation| {
                            Value::string(documentation.as_str())
                        })),
                    other => Err(RuntimeError::Type {
                        expected: "PACKAGE".to_string(),
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
                    state.use_package(&package, &target);
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
                Ok(Value::boolean(self.constantp(&arguments[0])))
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
                if !properties.len().is_multiple_of(2) {
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
                if !properties.len().is_multiple_of(2) {
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
                if !properties.len().is_multiple_of(2) {
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
                self.apply_sequence_substitute(SequenceSubstituteRequest {
                    operation: name,
                    new_item: &arguments[0],
                    old_or_predicate: &arguments[1],
                    sequence: &arguments[2],
                    options: &arguments[3..],
                    environment,
                    span,
                })
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
                self.apply_sequence_merge(SequenceMergeRequest {
                    result_type: &arguments[0],
                    sequence1: &arguments[1],
                    sequence2: &arguments[2],
                    predicate: &arguments[3],
                    options: &arguments[4..],
                    environment,
                    span,
                })
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

    pub(super) fn method_score(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
    ) -> Option<usize> {
        let required_count = method.specializers.len();
        if arguments.len() < required_count {
            return None;
        }
        if let Value::Function(function) = &method.function
            && let crate::Function::Closure {
                parameters,
                optional,
                rest,
                has_keyword_section,
                ..
            } = function.as_ref()
            && (parameters.len() != required_count
                || (!*has_keyword_section
                    && rest.is_none()
                    && arguments.len() > required_count + optional.len()))
        {
            return None;
        }
        let mut score = 0usize;
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            if specializer == "T" || specializer == "OBJECT" {
                score = score.saturating_add(1_000_000);
                continue;
            }
            let class = argument.instance_class_definition()?;
            let position = class
                .precedence
                .iter()
                .position(|name| name == specializer)?;
            score = score.saturating_add(position);
        }
        Some(score)
    }

    pub(super) fn invoke_method(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
        next: Option<MethodContinuation>,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.method_context.borrow_mut().push(MethodContext {
            arguments: arguments.to_vec(),
            next,
        });
        let result = self.apply_in(&method.function, arguments, span, environment);
        self.method_context.borrow_mut().pop();
        result
    }

    pub(super) fn invoke_core(
        &self,
        before: &[MethodDefinition],
        primary: &[MethodDefinition],
        after: &[MethodDefinition],
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for method in before {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        let Some(method) = primary.first() else {
            return Err(self.invalid("no primary method is applicable", span));
        };
        let next = (primary.len() > 1).then(|| MethodContinuation::Chain {
            methods: primary.to_vec(),
            index: 1,
            fallback: None,
        });
        let result = self.invoke_method(method, arguments, next, span, environment)?;
        for method in after {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        Ok(result)
    }

    pub(super) fn invoke_continuation(
        &self,
        continuation: MethodContinuation,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match continuation {
            MethodContinuation::Chain {
                methods,
                index,
                fallback,
            } => {
                if index < methods.len() {
                    let method = methods[index].clone();
                    let next = if index + 1 < methods.len() || fallback.is_some() {
                        Some(MethodContinuation::Chain {
                            methods,
                            index: index + 1,
                            fallback,
                        })
                    } else {
                        None
                    };
                    self.invoke_method(&method, arguments, next, span, environment)
                } else if let Some(fallback) = fallback {
                    self.invoke_continuation(*fallback, arguments, span, environment)
                } else {
                    Err(self.invalid("no next method is applicable", span))
                }
            }
            MethodContinuation::Core {
                before,
                primary,
                after,
            } => self.invoke_core(&before, &primary, &after, arguments, span, environment),
        }
    }

    pub(super) fn apply_generic(
        &self,
        name: &str,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut applicable = methods
            .borrow()
            .iter()
            .filter_map(|method| {
                self.method_score(method, arguments)
                    .map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            return Err(self.invalid(&format!("no applicable method for {name}"), span));
        }
        applicable.sort_by_key(|(score, _)| *score);

        let mut around = Vec::new();
        let mut before = Vec::new();
        let mut primary = Vec::new();
        let mut after = Vec::new();
        for (_, method) in applicable {
            match method.qualifiers.first().map(String::as_str) {
                Some("AROUND") => around.push(method),
                Some("BEFORE") => before.push(method),
                Some("AFTER") => after.push(method),
                _ => primary.push(method),
            }
        }
        after.reverse();
        let core = MethodContinuation::Core {
            before,
            primary,
            after,
        };
        if around.is_empty() {
            self.invoke_continuation(core, arguments, span, environment)
        } else {
            let first = around[0].clone();
            let next = MethodContinuation::Chain {
                methods: around,
                index: 1,
                fallback: Some(Box::new(core)),
            };
            self.invoke_method(&first, arguments, Some(next), span, environment)
        }
    }

    pub(crate) fn apply_in(
        &self,
        function: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(function, span, environment)?;
        match function.as_ref() {
            crate::Function::Builtin { function, .. } => function(arguments),
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic { name, methods } => {
                self.apply_generic(name, methods, arguments, span, environment)
            }
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("slot reader", "one", arguments.len()));
                }
                if !arguments[0].instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                let value = arguments[0]
                    .instance_slot(slot_name)
                    .ok_or_else(|| self.invalid("slot is not defined for this class", span))?;
                if matches!(value, Value::Unbound) {
                    return Err(self.invalid("slot is unbound", span));
                }
                Ok(value)
            }
            crate::Function::SlotWriter {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if !object.instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: object.type_name().to_string(),
                        span: Some(span),
                    });
                }
                if object.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("slot is not defined for this class", span))
                }
            }
            crate::Function::ConditionReader {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("condition reader", "one", arguments.len()));
                }
                arguments[0]
                    .condition_slot(condition_name, slot_name)
                    .ok_or_else(|| self.invalid("condition slot is not defined", span))
            }
            crate::Function::ConditionWriter {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("condition writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if object.set_condition_slot(condition_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("condition slot is not defined", span))
                }
            }
            crate::Function::StructureConstructor {
                name,
                slots,
                structure_types,
                constructor_lambda_list,
                environment: definition_environment,
            } => {
                if let Some(lambda_list) = constructor_lambda_list {
                    self.apply_structure_boa_constructor(StructureBoaRequest {
                        name,
                        slots,
                        structure_types,
                        lambda_list,
                        definition_environment,
                        arguments,
                        span,
                    })
                } else {
                    if !arguments.len().is_multiple_of(2) {
                        return Err(self.arity(
                            "structure constructor",
                            "an even number of",
                            arguments.len(),
                        ));
                    }
                    let mut supplied = vec![None; slots.len()];
                    for pair in arguments.chunks_exact(2) {
                        let keyword_name = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => {
                                normalize_name(keyword)
                            }
                            _ => {
                                return Err(self.invalid(
                                    "structure constructor keyword name must be a keyword",
                                    span,
                                ));
                            }
                        };
                        let Some(index) = slots.iter().position(|slot| slot.name == keyword_name)
                        else {
                            return Err(RuntimeError::InvalidForm {
                                message: format!("unknown structure keyword :{keyword_name}"),
                                span: Some(span),
                            });
                        };
                        supplied[index] = Some(pair[1].clone());
                    }
                    let mut values = Vec::with_capacity(slots.len());
                    for (index, slot) in slots.iter().enumerate() {
                        let value = match supplied[index].clone() {
                            Some(value) => value,
                            None => slot
                                .init_form
                                .as_ref()
                                .map(|form| self.eval_in(form, definition_environment))
                                .transpose()?
                                .unwrap_or(Value::Nil),
                        };
                        values.push((slot.name.clone(), value));
                    }
                    Ok(Value::structure_with_types(
                        name,
                        values,
                        structure_types.clone(),
                    ))
                }
            }
            crate::Function::StructurePredicate { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure predicate", "one", arguments.len()));
                }
                Ok(Value::boolean(arguments[0].structure_is_type(name)))
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_name: _,
                slot_index,
                ..
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure accessor", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(structure_name) {
                    return Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .structure_slot(*slot_index)
                    .ok_or_else(|| self.invalid("structure slot is out of range", span))
            }
            crate::Function::StructureCopier { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure copier", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(name) {
                    return Err(RuntimeError::Type {
                        expected: name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .copy_structure()
                    .ok_or_else(|| self.invalid("structure copy failed", span))
            }
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
                environment,
            } => {
                let required_count = parameters.len();
                let optional_count = optional.len();
                let maximum_count = required_count + optional_count;
                if arguments.len() < required_count {
                    let expected = if optional_count > 0 || rest.is_some() || *has_keyword_section {
                        format!("at least {required_count}")
                    } else {
                        required_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }
                let optional_supplied_count = if *has_keyword_section {
                    let available = arguments
                        .len()
                        .saturating_sub(required_count)
                        .min(optional_count);
                    (0..available)
                        .take_while(|index| {
                            !matches!(
                                arguments[required_count + *index],
                                Value::Keyword(_) | Value::KeywordExact(_)
                            )
                        })
                        .count()
                } else {
                    arguments
                        .len()
                        .saturating_sub(required_count)
                        .min(optional_count)
                };
                let key_start = required_count + optional_supplied_count;
                if !*has_keyword_section && rest.is_none() && arguments.len() > maximum_count {
                    let expected = if optional_count > 0 {
                        format!("at most {maximum_count}")
                    } else {
                        maximum_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }

                let local = environment.child();
                let _dynamic_guard = self.dynamic_guard();
                for (index, (parameter, argument)) in
                    parameters.iter().zip(arguments.iter()).enumerate()
                {
                    if required_escaped.get(index).copied().unwrap_or(false) {
                        self.define_exact_in(parameter, argument.clone(), &local);
                    } else {
                        self.define_in(parameter, argument.clone(), &local);
                    }
                }
                for (index, specification) in optional.iter().enumerate() {
                    let supplied = (index < optional_supplied_count)
                        .then(|| &arguments[required_count + index]);
                    let value = match supplied {
                        Some(argument) => argument.clone(),
                        None => self.eval_in(&specification.init_form, &local)?,
                    };
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                    if let Some(supplied_p) = &specification.supplied_p {
                        if specification.supplied_p_escaped.unwrap_or(false) {
                            self.define_exact_in(
                                supplied_p,
                                Value::boolean(supplied.is_some()),
                                &local,
                            );
                        } else {
                            self.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                        }
                    }
                }
                if let Some(rest) = rest {
                    let rest_start = key_start;
                    let value = Value::list(arguments[rest_start..].to_vec());
                    if *rest_escaped {
                        self.define_exact_in(rest, value, &local);
                    } else {
                        self.define_in(rest, value, &local);
                    }
                }
                if *has_keyword_section {
                    let keyword_arguments = &arguments[key_start..];
                    if !keyword_arguments.len().is_multiple_of(2) {
                        return Err(
                            self.invalid("keyword arguments must be supplied in pairs", span)
                        );
                    }
                    let mut supplied_keywords = HashMap::new();
                    let mut accepts_unknown_keywords = *allow_other_keys;
                    for pair in keyword_arguments.chunks_exact(2) {
                        let keyword = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                            _ => {
                                return Err(
                                    self.invalid("keyword argument name must be a keyword", span)
                                );
                            }
                        };
                        let keyword_name = keyword.to_string();
                        if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                            accepts_unknown_keywords = true;
                        }
                        supplied_keywords.insert(keyword_name, pair[1].clone());
                    }
                    if !accepts_unknown_keywords {
                        for keyword_name in supplied_keywords.keys() {
                            if keyword_name != "ALLOW-OTHER-KEYS"
                                && !keywords.iter().any(|specification| {
                                    specification.keyword_name == *keyword_name
                                })
                            {
                                return Err(RuntimeError::InvalidForm {
                                    message: format!("unknown keyword :{keyword_name}"),
                                    span: Some(span),
                                });
                            }
                        }
                    }
                    for specification in keywords {
                        let supplied = supplied_keywords.get(&specification.keyword_name);
                        let value = match supplied {
                            Some(argument) => argument.clone(),
                            None => self.eval_in(&specification.init_form, &local)?,
                        };
                        if specification.name_escaped {
                            self.define_exact_in(&specification.name, value, &local);
                        } else {
                            self.define_in(&specification.name, value, &local);
                        }
                        if let Some(supplied_p) = &specification.supplied_p {
                            if specification.supplied_p_escaped.unwrap_or(false) {
                                self.define_exact_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            } else {
                                self.define_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            }
                        }
                    }
                }
                for specification in auxiliary {
                    let value = self.eval_in(&specification.init_form, &local)?;
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                }
                self.eval_sequence_values(body, &local)
            }
            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. } => {
                Err(RuntimeError::NotCallable {
                    value: Value::Function(function.clone()).to_string(),
                    span: Some(span),
                })
            }
            crate::Function::Compiled {
                program,
                function,
                environment,
            } => crate::vm::run(
                self,
                program.clone(),
                *function,
                environment.clone(),
                arguments,
                span,
            ),
        }
    }

    pub(super) fn apply_structure_boa_constructor(
        &self,
        request: StructureBoaRequest<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureBoaRequest {
            name,
            slots,
            structure_types,
            lambda_list,
            definition_environment,
            arguments,
            span,
        } = request;
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            let expected = if optional_count > 0
                || lambda_list.rest.is_some()
                || lambda_list.has_keyword_section
            {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
        }
        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && arguments.len() > required_count + optional_count
        {
            let maximum = required_count + optional_count;
            let expected = if optional_count > 0 {
                format!("at most {maximum}")
            } else {
                maximum.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
        }

        let local = definition_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut slot_values = vec![None; slots.len()];
        let slot_index =
            |parameter_name: &str| slots.iter().position(|slot| slot.name == parameter_name);
        let evaluate_slot_default = |parameter_name: &str| -> Result<Value, RuntimeError> {
            slots
                .iter()
                .find(|slot| slot.name == parameter_name)
                .and_then(|slot| slot.init_form.as_ref())
                .map(|form| self.eval_in(form, definition_environment))
                .transpose()
                .map(|value| value.unwrap_or(Value::Nil))
        };

        for (index, (parameter, argument)) in lambda_list
            .required
            .iter()
            .zip(arguments.iter())
            .enumerate()
        {
            if lambda_list
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }

        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, &local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, &local);
                } else {
                    self.define_in(supplied_p, supplied_value, &local);
                }
            }
        }

        if let Some(rest) = &lambda_list.rest {
            let value = Value::list(arguments[key_start..].to_vec());
            if lambda_list.rest_escaped {
                self.define_exact_in(rest, value.clone(), &local);
            } else {
                self.define_in(rest, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(rest) {
                slot_values[slot_index] = Some(value);
            }
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if !keyword_arguments.len().is_multiple_of(2) {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = Vec::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let keyword_name = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword.to_string(),
                    _ => return Err(self.invalid("keyword argument name must be a keyword", span)),
                };
                if normalize_name(&keyword_name) == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.push((keyword_name, pair[1].clone()));
            }
            let keyword_matches = |specification: &LambdaListKeywordParameter,
                                   actual_name: &str| {
                if specification.keyword_name_escaped {
                    specification.keyword_name == actual_name
                } else {
                    normalize_name(&specification.keyword_name) == normalize_name(actual_name)
                }
            };
            if !accepts_unknown_keywords {
                for (keyword_name, _) in &supplied_keywords {
                    if normalize_name(keyword_name) != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| keyword_matches(specification, keyword_name))
                    {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(span),
                        });
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords
                    .iter()
                    .rev()
                    .find(|(keyword_name, _)| keyword_matches(specification, keyword_name));
                let value = match supplied {
                    Some((_, argument)) => argument.clone(),
                    None if specification.init_form_supplied => {
                        self.eval_in(&specification.init_form, &local)?
                    }
                    None => evaluate_slot_default(&specification.name)?,
                };
                if specification.name_escaped {
                    self.define_exact_in(&specification.name, value.clone(), &local);
                } else {
                    self.define_in(&specification.name, value.clone(), &local);
                }
                if let Some(slot_index) = slot_index(&specification.name) {
                    slot_values[slot_index] = Some(value);
                }
                if let Some(supplied_p) = &specification.supplied_p {
                    let supplied_value = Value::boolean(supplied.is_some());
                    if specification.supplied_p_escaped.unwrap_or(false) {
                        self.define_exact_in(supplied_p, supplied_value, &local);
                    } else {
                        self.define_in(supplied_p, supplied_value, &local);
                    }
                }
            }
        }

        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match slot_values[index].take() {
                Some(value) => value,
                None => evaluate_slot_default(&slot.name)?,
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }
}

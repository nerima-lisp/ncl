#![allow(clippy::wildcard_imports)]
use super::evaluator_special_forms::evaluator_sequences::sequence_types::SequenceSubstituteContext;
use super::*;

impl Runtime {
    pub(crate) fn apply_evaluation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "EVAL" | "COMPILE" | "LOAD" | "MAKE-INSTANCE") {
            return None;
        }
        let result = match name {
            "EVAL" => match arguments.len() {
                1 => Self::form_from_value(&arguments[0], span)
                    .and_then(|form| self.eval_values_in(&form, environment)),
                _ => Err(Self::arity("eval", "one", arguments.len())),
            },
            "COMPILE" => self.compile_function(arguments, environment, span),
            "LOAD" => self.load_file(arguments, span),
            "MAKE-INSTANCE" => self.make_instance(arguments, environment, span),
            _ => unreachable!("evaluation primitive name was prevalidated"),
        };
        Some(result)
    }

    pub(crate) fn apply_condition_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        let result = match name {
            "ERROR" => self.primitive_error(arguments, environment, span),
            "SIGNAL" => self.primitive_signal(arguments, environment, span),
            "WARN" => self.primitive_warn(arguments, environment, span),
            "CERROR" => self.primitive_cerror(arguments, environment, span),
            "MAKE-CONDITION" => Self::make_condition(arguments, span),
            _ => return None,
        };
        Some(result)
    }

    fn primitive_error(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("error", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            let error = Self::condition_error(&arguments[0], false, span)?;
            return match self.dispatch_condition(error.clone(), &arguments[0], environment, span) {
                Ok(()) | Err(_) => Err(error),
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
            message,
            format_control,
            format_arguments,
            false,
            environment,
            span,
        ) {
            Ok(()) => Err(error),
            Err(error) => Err(error),
        }
    }

    fn primitive_signal(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("signal", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            if arguments.len() != 1 {
                return Err(Self::invalid(
                    "signal does not accept format arguments with a condition object",
                    span,
                ));
            }
            self.signal_condition_value(&arguments[0], false, environment, span)?;
            return Ok(Value::Nil);
        }
        let format_arguments = &arguments[1..];
        self.signal_condition(
            "SIMPLE-CONDITION",
            Self::condition_message(&arguments[0], format_arguments, span)?,
            Self::condition_format_control(&arguments[0]),
            format_arguments,
            false,
            environment,
            span,
        )?;
        Ok(Value::Nil)
    }

    fn primitive_warn(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("warn", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            if arguments.len() != 1 {
                return Err(Self::invalid(
                    "warn does not accept format arguments with a condition object",
                    span,
                ));
            }
            self.signal_condition_value(&arguments[0], true, environment, span)?;
            return Ok(Value::Nil);
        }
        let format_arguments = &arguments[1..];
        self.signal_condition(
            "SIMPLE-WARNING",
            Self::condition_message(&arguments[0], format_arguments, span)?,
            Self::condition_format_control(&arguments[0]),
            format_arguments,
            true,
            environment,
            span,
        )?;
        Ok(Value::Nil)
    }

    fn primitive_cerror(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() < 2 {
            return Err(Self::arity("cerror", "at least two", arguments.len()));
        }
        let format_arguments = &arguments[2..];
        let _continue_message = Self::condition_message(&arguments[0], format_arguments, span)?;
        let condition_object = arguments[1].condition_type_name().is_some();
        if condition_object && !format_arguments.is_empty() {
            return Err(Self::invalid(
                "cerror does not accept format arguments with a condition object",
                span,
            ));
        }
        let format_control = Self::condition_format_control(&arguments[1]);
        let message = Self::condition_message(&arguments[1], format_arguments, span)?;
        let result = if condition_object {
            self.dispatch_condition(
                Self::condition_error(&arguments[1], false, span)?,
                &arguments[1],
                environment,
                span,
            )
        } else {
            self.signal_condition(
                "SIMPLE-ERROR",
                message.clone(),
                format_control,
                format_arguments,
                false,
                environment,
                span,
            )
        };
        match result {
            Ok(()) => {}
            Err(RuntimeError::InvokeRestart { name, .. })
                if normalize_name(&name) == "CONTINUE" =>
            {
                return Ok(Value::Nil);
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
            Err(Self::invalid(&message, span))
        }
    }

    pub(crate) fn apply_package_use_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(
            name,
            "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT"
        ) {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            if arguments.len() != 1 && arguments.len() != 2 {
                return Err(Self::arity(name, "one or two", arguments.len()));
            }
            let target = arguments
                .get(1)
                .map(|value| self.package_name_from_value(value, span))
                .transpose()?
                .unwrap_or_else(|| self.current_package());
            match name {
                "USE-PACKAGE" | "UNUSE-PACKAGE" => {
                    let packages = self.package_names_from_value(&arguments[0], span)?;
                    if name == "USE-PACKAGE" && packages.iter().any(|package| package == &target) {
                        return Err(Self::package_error("a package cannot use itself", span));
                    }
                    let mut state = self.packages.borrow_mut();
                    for package in packages {
                        match name {
                            "USE-PACKAGE" => state.use_package(&package, &target),
                            "UNUSE-PACKAGE" => state.unuse_package(&package, &target),
                            _ => unreachable!("package use primitive name was prevalidated"),
                        }
                    }
                    Ok(Value::boolean(true))
                }
                "EXPORT" | "UNEXPORT" => {
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
                    let mut state = self.packages.borrow_mut();
                    if name == "EXPORT" {
                        state.export_symbols(&target, &symbols);
                    } else {
                        state.unexport_symbols(&target, &symbols);
                    }
                    Ok(Value::boolean(true))
                }
                _ => unreachable!("package use primitive name was prevalidated"),
            }
        })();
        Some(result)
    }

    pub(crate) fn apply_package_symbol_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN") {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            if arguments.len() != 1 && arguments.len() != 2 {
                return Err(Self::arity(name, "one or two", arguments.len()));
            }
            let target = arguments
                .get(1)
                .map(|value| self.package_name_from_value(value, span))
                .transpose()?
                .unwrap_or_else(|| self.current_package());
            match name {
                "IMPORT" | "SHADOWING-IMPORT" => {
                    let imports = self.symbol_import_references_from_value(&arguments[0], span)?;
                    {
                        let state = self.packages.borrow();
                        for (source_package, source_name) in &imports {
                            if !state.symbol_exists(source_package, source_name) {
                                return Err(Self::package_error(
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
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
                    let mut state = self.packages.borrow_mut();
                    for symbol in symbols {
                        state.shadow_symbol(&target, &symbol);
                    }
                    Ok(Value::boolean(true))
                }
                "UNINTERN" => {
                    let symbols = Self::symbol_names_from_value(&arguments[0], span)?;
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
                _ => unreachable!("package symbol primitive name was prevalidated"),
            }
        })();
        Some(result)
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

    pub(crate) fn apply_symbol_value_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "BOUNDP" | "CONSTANTP" | "SYMBOL-VALUE") {
            return None;
        }
        let result = (|| -> Result<Value, RuntimeError> {
            match name {
                "BOUNDP" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("boundp", "one", arguments.len()));
                    }
                    let (name, exact) = arguments[0]
                        .symbol_reference()
                        .ok_or_else(|| Self::invalid("boundp argument must be a symbol", span))?;
                    Ok(Value::boolean(if exact {
                        self.is_bound_exact_in(name, environment)
                    } else {
                        self.is_bound_in(name, environment)
                    }))
                }
                "CONSTANTP" => {
                    if arguments.len() != 1 && arguments.len() != 2 {
                        return Err(Self::arity("constantp", "one or two", arguments.len()));
                    }
                    Ok(Value::boolean(self.constantp(&arguments[0])))
                }
                "SYMBOL-VALUE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("symbol-value", "one", arguments.len()));
                    }
                    let (name, exact) = arguments[0].symbol_reference().ok_or_else(|| {
                        Self::invalid("symbol-value argument must be a symbol", span)
                    })?;
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
                _ => unreachable!("symbol value name was prevalidated"),
            }
        })();
        Some(result)
    }

    pub(crate) fn apply_symbol_function_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        match name {
            "FBOUNDP" => Some(self.apply_fboundp(arguments, environment, span)),
            "MACRO-FUNCTION" => Some(self.apply_macro_function(arguments, span)),
            "SPECIAL-OPERATOR-P" => Some(Self::apply_special_operator_p(arguments, span)),
            "COMPILED-FUNCTION-P" => Some(Self::apply_compiled_function_p(arguments)),
            "FDEFINITION" | "SYMBOL-FUNCTION" => {
                Some(self.apply_function_definition(name, arguments, environment, span))
            }
            _ => None,
        }
    }

    fn apply_fboundp(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("fboundp", "one", arguments.len()));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("fboundp argument must be a symbol", span))?;
        let value = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        Ok(Value::boolean(matches!(value, Some(Value::Function(_)))))
    }

    fn apply_macro_function(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 && arguments.len() != 2 {
            return Err(Self::arity("macro-function", "one or two", arguments.len()));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("macro-function argument must be a symbol", span))?;
        let environment = match arguments.get(1) {
            None | Some(Value::Nil | Value::Boolean(false)) => &self.global,
            Some(Value::Environment(environment)) => environment,
            Some(_) => {
                return Err(Self::invalid(
                    "macro-function environment must be an environment",
                    span,
                ));
            }
        };
        let value = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
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

    fn apply_special_operator_p(arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("special-operator-p", "one", arguments.len()));
        }
        let (name, _) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("special-operator-p argument must be a symbol", span))?;
        Ok(Value::boolean(is_special_operator_name(name)))
    }

    fn apply_compiled_function_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("compiled-function-p", "one", arguments.len()));
        }
        Ok(Value::boolean(matches!(
            &arguments[0],
            Value::Function(function) if matches!(function.as_ref(), crate::Function::Compiled { .. })
        )))
    }

    fn apply_function_definition(
        &self,
        primitive: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity(
                &primitive.to_ascii_lowercase(),
                "one",
                arguments.len(),
            ));
        }
        let (name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| Self::invalid("function argument must be a symbol", span))?;
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

    pub(crate) fn apply_package_introspection_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "FIND-PACKAGE" | "PACKAGE-NAME" | "PACKAGE-USE-LIST") {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "FIND-PACKAGE" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("find-package", "one", arguments.len()));
                    }
                    let package = Self::package_designator_name(&arguments[0], span)?;
                    let packages = self.packages.borrow();
                    Ok(if packages.package_exists(&package) {
                        Value::package(packages.canonical_package_name(&package))
                    } else {
                        Value::Nil
                    })
                }
                "PACKAGE-NAME" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-name", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::string(package.as_ref())),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                "PACKAGE-USE-LIST" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("package-use-list", "one", arguments.len()));
                    }
                    match &arguments[0] {
                        Value::Package(package) => Ok(Value::list(
                            self.packages
                                .borrow()
                                .use_packages_for(package)
                                .into_iter()
                                .map(Value::package)
                                .collect(),
                        )),
                        other => Err(RuntimeError::Type {
                            expected: "PACKAGE".into(),
                            actual: other.type_name().into(),
                            span: Some(span),
                        }),
                    }
                }
                _ => unreachable!(),
            }
        })())
    }

    pub(crate) fn apply_symbol_creation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        if !matches!(name, "MAKE-SYMBOL" | "GENSYM" | "INTERN" | "FIND-SYMBOL") {
            return None;
        }
        Some((|| -> Result<Value, RuntimeError> {
            match name {
                "MAKE-SYMBOL" => {
                    if arguments.len() != 1 {
                        return Err(Self::arity("make-symbol", "one", arguments.len()));
                    }
                    let Value::String(value) = &arguments[0] else {
                        return Err(Self::invalid("make-symbol argument must be a string", span));
                    };
                    Ok(Value::uninterned_symbol(value.as_ref()))
                }
                "GENSYM" => {
                    if arguments.len() > 1 {
                        return Err(Self::arity("gensym", "zero or one", arguments.len()));
                    }
                    let prefix = match arguments.first() {
                        None => "G".into(),
                        Some(Value::String(v)) => v.to_string(),
                        Some(v) => v.symbol_name().map(str::to_owned).ok_or_else(|| {
                            Self::invalid("gensym prefix must be a string designator", span)
                        })?,
                    };
                    let counter = self.gensym_counter.get();
                    self.gensym_counter.set(counter.wrapping_add(1));
                    Ok(Value::uninterned_symbol(format!("{prefix}{counter}")))
                }
                "INTERN" | "FIND-SYMBOL" => {
                    if !(1..=2).contains(&arguments.len()) {
                        return Err(Self::arity(
                            &name.to_ascii_lowercase(),
                            "one or two",
                            arguments.len(),
                        ));
                    }
                    let symbol_name = Self::symbol_name_from_value(&arguments[0], span)?;
                    let package_name = arguments
                        .get(1)
                        .map(|v| self.package_name_from_value(v, span))
                        .transpose()?
                        .unwrap_or_else(|| self.current_package());
                    if name == "INTERN" {
                        let Some(status) = self
                            .packages
                            .borrow_mut()
                            .intern_symbol(&package_name, &symbol_name)
                        else {
                            return Err(Self::package_error(
                                &format!("unknown package {package_name}"),
                                span,
                            ));
                        };
                        Ok(Value::values(vec![
                            self.package_symbol_value(&package_name, &symbol_name),
                            Self::symbol_status_value(status),
                        ]))
                    } else {
                        self.packages
                            .borrow()
                            .symbol_status(&package_name, &symbol_name)
                            .map_or_else(
                                || Ok(Value::values(vec![Value::Nil, Value::Nil])),
                                |status| {
                                    Ok(Value::values(vec![
                                        self.package_symbol_value(&package_name, &symbol_name),
                                        Self::symbol_status_value(status),
                                    ]))
                                },
                            )
                    }
                }
                _ => unreachable!(),
            }
        })())
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
                            precedence: vec![n, "STANDARD-OBJECT".into()],
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
                _ => unreachable!(),
            }
        })())
    }

    pub(crate) fn apply_restart_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "COMPUTE-RESTARTS" => {
                if arguments.len() > 1 {
                    return Some(Err(Self::arity(
                        "compute-restarts",
                        "at most one",
                        arguments.len(),
                    )));
                }
                let condition = arguments
                    .first()
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Some(Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    }));
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
                    return Some(Err(Self::arity(
                        "find-restart",
                        "one or two",
                        arguments.len(),
                    )));
                }
                let condition = arguments
                    .get(1)
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Some(Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    }));
                }
                let bindings = self.restart_bindings_for_condition(condition);
                match Self::restart_binding_for_designator_in(&arguments[0], &bindings, span) {
                    Ok(binding) => Ok(binding.map_or(Value::Nil, |binding| binding.restart)),
                    Err(error) => Err(error),
                }
            }
            "RESTART-NAME" => {
                if arguments.len() != 1 {
                    return Some(Err(Self::arity("restart-name", "one", arguments.len())));
                }
                let Some(name) = arguments[0].restart_name() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "RESTART".to_string(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    }));
                };
                Ok(Value::symbol(name))
            }
            "INVOKE-RESTART" => {
                if arguments.is_empty() {
                    return Some(Err(Self::arity(
                        "invoke-restart",
                        "at least one",
                        arguments.len(),
                    )));
                }
                if let Some((name, _)) = arguments[0].symbol_reference() {
                    return Some(self.invoke_restart_named(
                        name,
                        &arguments[1..],
                        environment,
                        span,
                    ));
                }
                let binding = match self.restart_binding_for_designator(&arguments[0], span) {
                    Ok(Some(binding)) => binding,
                    Ok(None) => return Some(Err(Self::invalid("restart is not active", span))),
                    Err(error) => return Some(Err(error)),
                };
                self.invoke_restart_binding(binding, &arguments[1..], environment, span)
            }
            _ => return None,
        })
    }

    pub(crate) fn apply_sequence_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        self.apply_sequence_mutation_primitive(name, arguments, environment, span)
            .or_else(|| self.apply_sequence_set_primitive(name, arguments, environment, span))
            .or_else(|| self.apply_sequence_search_primitive(name, arguments, environment, span))
    }

    fn apply_sequence_mutation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF" | "DELETE-IF-NOT"
                if arguments.len() >= 2 =>
            {
                self.apply_sequence_remove(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" if !arguments.is_empty() => self
                .apply_sequence_remove(
                    name,
                    &Value::Nil,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                ),
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least one",
                arguments.len(),
            )),
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT"
                if arguments.len() >= 3 =>
            {
                self.apply_sequence_substitute(SequenceSubstituteContext {
                    operation: name,
                    new_item: &arguments[0],
                    old_or_predicate: &arguments[1],
                    sequence: &arguments[2],
                    options: &arguments[3..],
                    environment,
                    span,
                })
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least three",
                arguments.len(),
            )),
            _ => return None,
        })
    }

    fn apply_sequence_set_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP"
                if arguments.len() >= 2 =>
            {
                self.apply_list_set_operation(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "UNION" | "NUNION" | "INTERSECTION" | "NINTERSECTION" | "SET-DIFFERENCE"
            | "NSET-DIFFERENCE" | "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" | "SUBSETP" => Err(
                Self::arity(&name.to_ascii_lowercase(), "at least two", arguments.len()),
            ),
            _ => return None,
        })
    }

    fn apply_sequence_search_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" if arguments.len() >= 2 => self
                .apply_list_membership(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                ),
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT"
                if arguments.len() >= 2 =>
            {
                self.apply_association_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "FIND" | "POSITION" | "COUNT" if arguments.len() >= 2 => self.apply_sequence_search(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "SEARCH" | "MISMATCH" if arguments.len() >= 2 => self.apply_sequence_pair_search(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "SORT" | "STABLE-SORT" if arguments.len() >= 2 => self.apply_sequence_sort(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" if arguments.len() >= 2 => self
                .apply_sequence_quantifier(name, &arguments[0], &arguments[1..], environment, span),
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON"
                if arguments.len() >= 2 =>
            {
                self.apply_list_mapping(name, &arguments[0], &arguments[1..], environment, span)
            }
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" | "ASSOC" | "ASSOC-IF"
            | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT" | "FIND" | "POSITION"
            | "COUNT" | "SEARCH" | "MISMATCH" | "SORT" | "STABLE-SORT" | "EVERY" | "SOME"
            | "NOTANY" | "NOTEVERY" | "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN"
            | "MAPCON" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least two",
                arguments.len(),
            )),
            _ => return None,
        })
    }

    pub(crate) fn apply_symbol_property_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        match name {
            "GET" => Some(Self::apply_get_property(arguments, environment, span)),
            "PUTPROP" => Some(Self::apply_put_property(arguments, environment, span)),
            "REMPROP" => Some(Self::apply_rem_property(arguments, environment, span)),
            "SYMBOL-PLIST" => Some(Self::apply_symbol_plist(arguments, environment, span)),
            "SET" => Some(self.apply_symbol_set(arguments, span)),
            "MAKUNBOUND" | "FMAKUNBOUND" => Some(self.apply_symbol_unbound(name, arguments, span)),
            _ => None,
        }
    }

    fn apply_get_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&arguments.len()) {
            return Err(Self::arity("get", "two or three", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid("get first argument must be a symbol", span));
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
            return Err(Self::invalid("GET needs an even property list", span));
        }
        for index in (0..properties.len()).step_by(2) {
            if properties[index].eq_value(&arguments[1]) {
                return Ok(properties[index + 1].clone());
            }
        }
        Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
    }

    fn apply_put_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 3 {
            return Err(Self::arity("putprop", "three", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "putprop first argument must be a symbol",
                span,
            ));
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
            return Err(Self::invalid("PUTPROP needs an even property list", span));
        }
        if let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|index| properties[*index].eq_value(&arguments[2]))
        {
            properties[index] = arguments[1].clone();
        } else {
            properties.extend([arguments[2].clone(), arguments[1].clone()]);
        }
        environment.set_symbol_plist(&arguments[0], Value::list(properties));
        Ok(arguments[1].clone())
    }

    fn apply_rem_property(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("remprop", "two", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "remprop first argument must be a symbol",
                span,
            ));
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
            return Err(Self::invalid("REMPROP needs an even property list", span));
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

    fn apply_symbol_plist(
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("symbol-plist", "one", arguments.len()));
        }
        if arguments[0].symbol_reference().is_none() {
            return Err(Self::invalid(
                "symbol-plist argument must be a symbol",
                span,
            ));
        }
        Ok(environment
            .symbol_plist(&arguments[0])
            .unwrap_or(Value::Nil))
    }

    fn apply_symbol_set(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("set", "two", arguments.len()));
        }
        let Some((name, exact)) = arguments[0].symbol_reference() else {
            return Err(Self::invalid("set first argument must be a symbol", span));
        };
        self.ensure_symbol_writable(name, exact, span)?;
        Ok(if exact {
            self.set_symbol_value_exact(name, arguments[1].clone())
        } else {
            self.set_symbol_value(name, arguments[1].clone())
        })
    }

    fn apply_symbol_unbound(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity(
                &name.to_ascii_lowercase(),
                "one",
                arguments.len(),
            ));
        }
        let Some((symbol_name, exact)) = arguments[0].symbol_reference() else {
            return Err(Self::invalid(
                "unbound operation argument must be a symbol",
                span,
            ));
        };
        if name == "MAKUNBOUND" {
            self.ensure_symbol_writable(symbol_name, exact, span)?;
            if exact {
                self.makunbound_exact_symbol(symbol_name);
            } else {
                self.makunbound_symbol(symbol_name);
            }
        } else if exact {
            self.fmakunbound_exact_symbol(symbol_name);
        } else {
            self.fmakunbound_symbol(symbol_name);
        }
        Ok(arguments[0].clone())
    }
}

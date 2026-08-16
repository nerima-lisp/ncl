macro_rules! evaluator_generic {
    () => {
    fn method_score(&self, method: &MethodDefinition, arguments: &[Value]) -> Option<Vec<usize>> {
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
        let mut score = Vec::with_capacity(required_count);
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            match specializer {
                MethodSpecializer::Eql(value) => {
                    if !builtins::eql_value(value, argument) {
                        return None;
                    }
                    score.push(0);
                }
                MethodSpecializer::Class(class_name) => {
                    if class_name == "T" || class_name == "OBJECT" {
                        score.push(1_000_000);
                        continue;
                    }
                    let class = argument.instance_class_definition()?;
                    let position = class
                        .precedence
                        .iter()
                        .position(|name| name == class_name)?;
                    score.push(position.saturating_add(1));
                }
            }
        }
        Some(score)
    }

    fn same_method_identity(
        &self,
        existing: &MethodDefinition,
        candidate: &MethodDefinition,
    ) -> bool {
        existing.qualifiers == candidate.qualifiers
            && existing.specializers.len() == candidate.specializers.len()
            && existing
                .specializers
                .iter()
                .zip(candidate.specializers.iter())
                .all(|(left, right)| match (left, right) {
                    (MethodSpecializer::Class(left), MethodSpecializer::Class(right)) => {
                        left == right
                    }
                    (MethodSpecializer::Eql(left), MethodSpecializer::Eql(right)) => {
                        builtins::eql_value(left, right)
                    }
                    _ => false,
                })
    }

    fn ordered_applicable_methods(
        &self,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
    ) -> Vec<MethodDefinition> {
        let mut applicable = methods
            .borrow()
            .iter()
            .filter_map(|method| {
                self.method_score(method, arguments)
                    .map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        applicable.sort_by(|(left_score, _), (right_score, _)| left_score.cmp(right_score));
        applicable.into_iter().map(|(_, method)| method).collect()
    }

    fn invoke_method(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
        dispatch: &GenericDispatch,
        next: Option<MethodContinuation>,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.method_context.borrow_mut().push(MethodContext {
            dispatch: dispatch.clone(),
            method: method.function.clone(),
            arguments: arguments.to_vec(),
            next,
        });
        let result = self.apply_in(&method.function, arguments, span, environment);
        self.method_context.borrow_mut().pop();
        result
    }

    fn invoke_hook_in(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Option<Result<Value, RuntimeError>> {
        environment
            .lookup_function(name)
            .map(|function| self.apply_in(&function, arguments, span, environment))
    }

    fn no_applicable_method(
        &self,
        dispatch: &GenericDispatch,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !dispatch.name.eq_ignore_ascii_case("NO-APPLICABLE-METHOD") {
            let mut hook_arguments = Vec::with_capacity(arguments.len() + 1);
            hook_arguments.push(dispatch.function.clone());
            hook_arguments.extend(arguments.iter().cloned());
            if let Some(result) =
                self.invoke_hook_in("NO-APPLICABLE-METHOD", &hook_arguments, span, environment)
            {
                return result;
            }
        }
        Err(self.invalid(&format!("no applicable method for {}", dispatch.name), span))
    }

    fn no_next_method(
        &self,
        dispatch: &GenericDispatch,
        method: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut hook_arguments = Vec::with_capacity(arguments.len() + 2);
        hook_arguments.push(dispatch.function.clone());
        hook_arguments.push(method.clone());
        hook_arguments.extend(arguments.iter().cloned());
        if let Some(result) =
            self.invoke_hook_in("NO-NEXT-METHOD", &hook_arguments, span, environment)
        {
            return result;
        }
        Err(self.invalid("no next method is applicable", span))
    }

    fn execute_generic_default(
        &self,
        default: &GenericDefaultAction,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match default {
            GenericDefaultAction::Value(value) => Ok(value.clone()),
            GenericDefaultAction::SharedInitialize {
                instance,
                class,
                slot_names,
                initargs,
                unknown_initarg_message,
            } => {
                self.shared_initialize_instance(
                    instance,
                    class,
                    slot_names,
                    initargs,
                    EvaluationContext { environment, span },
                    unknown_initarg_message,
                )?;
                Ok(instance.clone())
            }
        }
    }

    fn invoke_core(&self, invocation: CoreMethodInvocation<'_>) -> Result<Value, RuntimeError> {
        let CoreMethodInvocation {
            dispatch,
            before,
            primary,
            after,
            default,
            arguments,
            context: EvaluationContext { environment, span },
        } = invocation;
        for method in before {
            self.invoke_method(method, arguments, dispatch, None, span, environment)?;
        }
        let result = if let Some(method) = primary.first() {
            let fallback = default
                .cloned()
                .map(MethodContinuation::Default)
                .map(Box::new);
            let next = if primary.len() > 1 || fallback.is_some() {
                Some(MethodContinuation::Chain {
                    dispatch: dispatch.clone(),
                    methods: primary.to_vec(),
                    index: 1,
                    fallback,
                })
            } else {
                None
            };
            self.invoke_method(method, arguments, dispatch, next, span, environment)?
        } else if let Some(default) = default {
            self.execute_generic_default(default, environment, span)?
        } else {
            return Err(self.invalid("no primary method is applicable", span));
        };
        for method in after {
            self.invoke_method(method, arguments, dispatch, None, span, environment)?;
        }
        Ok(result)
    }

    fn invoke_continuation(
        &self,
        continuation: MethodContinuation,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match continuation {
            MethodContinuation::Chain {
                dispatch,
                methods,
                index,
                fallback,
            } => {
                if index < methods.len() {
                    let method = methods[index].clone();
                    let next = if index + 1 < methods.len() || fallback.is_some() {
                        Some(MethodContinuation::Chain {
                            dispatch: dispatch.clone(),
                            methods,
                            index: index + 1,
                            fallback,
                        })
                    } else {
                        None
                    };
                    self.invoke_method(&method, arguments, &dispatch, next, span, environment)
                } else if let Some(fallback) = fallback {
                    self.invoke_continuation(*fallback, arguments, span, environment)
                } else {
                    Err(self.invalid("no next method is applicable", span))
                }
            }
            MethodContinuation::Core {
                dispatch,
                before,
                primary,
                after,
                default,
            } => self.invoke_core(CoreMethodInvocation {
                dispatch: &dispatch,
                before: &before,
                primary: &primary,
                after: &after,
                default: default.as_ref(),
                arguments,
                context: EvaluationContext { environment, span },
            }),
            MethodContinuation::Default(default) => {
                self.execute_generic_default(&default, environment, span)
            }
        }
    }

    fn apply_generic(
        &self,
        function: &Rc<crate::Function>,
        name: &str,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let default = if name.eq_ignore_ascii_case("INITIALIZE-INSTANCE") {
            if arguments.is_empty() {
                return Err(self.arity("initialize-instance", "at least one", arguments.len()));
            }
            if !(arguments.len() - 1).is_multiple_of(2) {
                return Err(self.invalid("initialize-instance initargs require pairs", span));
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
            Some(GenericDefaultAction::SharedInitialize {
                instance: arguments[0].clone(),
                class,
                slot_names: Value::Boolean(true),
                initargs,
                unknown_initarg_message: "unknown initialize-instance initarg",
            })
        } else if name.eq_ignore_ascii_case("REINITIALIZE-INSTANCE") {
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
            Some(GenericDefaultAction::SharedInitialize {
                instance: arguments[0].clone(),
                class,
                slot_names: Value::Nil,
                initargs,
                unknown_initarg_message: "unknown reinitialize-instance initarg",
            })
        } else {
            None
        };
        if name.eq_ignore_ascii_case("CHANGE-CLASS") {
            return self.change_class(arguments, environment, span);
        }
        self.apply_generic_with_default(
            function,
            name,
            methods,
            arguments,
            default,
            EvaluationContext { environment, span },
        )
    }

    fn apply_generic_with_default(
        &self,
        function: &Rc<crate::Function>,
        name: &str,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        default: Option<GenericDefaultAction>,
        context: EvaluationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let EvaluationContext { environment, span } = context;
        let applicable = self.ordered_applicable_methods(methods, arguments);
        let dispatch = GenericDispatch {
            name: name.to_owned(),
            function: Value::Function(function.clone()),
            methods: Rc::new(RefCell::new(methods.borrow().clone())),
            applicable: applicable.clone(),
        };
        if applicable.is_empty() {
            return if let Some(default) = default.as_ref() {
                self.execute_generic_default(default, environment, span)
            } else {
                self.no_applicable_method(&dispatch, arguments, span, environment)
            };
        }
        let mut around = Vec::new();
        let mut before = Vec::new();
        let mut primary = Vec::new();
        let mut after = Vec::new();
        for method in applicable {
            match method.qualifiers.first().map(String::as_str) {
                Some("AROUND") => around.push(method),
                Some("BEFORE") => before.push(method),
                Some("AFTER") => after.push(method),
                _ => primary.push(method),
            }
        }
        after.reverse();
        let core = MethodContinuation::Core {
            dispatch: dispatch.clone(),
            before,
            primary,
            after,
            default,
        };
        if around.is_empty() {
            self.invoke_continuation(core, arguments, span, environment)
        } else {
            let first = around[0].clone();
            let next = MethodContinuation::Chain {
                dispatch: dispatch.clone(),
                methods: around,
                index: 1,
                fallback: Some(Box::new(core)),
            };
            self.invoke_method(&first, arguments, &dispatch, Some(next), span, environment)
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
            crate::Function::Builtin { name, function } => {
                if name.eq_ignore_ascii_case("TYPEP") {
                    if arguments.len() != 2 {
                        return Err(self.arity("typep", "two", arguments.len()));
                    }
                    return Ok(Value::boolean(builtins::typep_value_in(
                        &arguments[0],
                        &arguments[1],
                        environment,
                    )?));
                }
                function(arguments)
            }
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic { name, methods, .. } => {
                self.apply_generic(&function, name, methods, arguments, span, environment)
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
                let class =
                    arguments[0]
                        .instance_class_definition()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: arguments[0].type_name().to_string(),
                            span: Some(span),
                        })?;
                let Some(value) = arguments[0].instance_slot(slot_name) else {
                    return self.slot_missing(
                        class,
                        &arguments[0],
                        slot_name,
                        "SLOT-VALUE",
                        None,
                        EvaluationContext { environment, span },
                    );
                };
                if matches!(value, Value::Unbound) {
                    return self.slot_unbound(class, &arguments[0], slot_name, environment, span);
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
                let class =
                    object
                        .instance_class_definition()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: object.type_name().to_string(),
                            span: Some(span),
                        })?;
                if object.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    self.slot_missing(
                        class,
                        object,
                        slot_name,
                        "SETF",
                        Some(value),
                        EvaluationContext { environment, span },
                    )
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
                    self.apply_structure_boa_constructor(StructureConstructorInvocation {
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
            crate::Function::Macro { .. }
            | crate::Function::LongDefsetf { .. }
            | crate::Function::ModifyMacro { .. } => Err(RuntimeError::NotCallable {
                value: Value::Function(function.clone()).to_string(),
                span: Some(span),
            }),
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

    fn apply_structure_boa_constructor(
        &self,
        invocation: StructureConstructorInvocation<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureConstructorInvocation {
            name,
            slots,
            structure_types,
            lambda_list,
            definition_environment,
            arguments,
            span,
        } = invocation;
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


    };
}

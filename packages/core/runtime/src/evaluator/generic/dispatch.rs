impl Runtime {
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
}

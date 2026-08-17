impl Runtime {
    fn find_method(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(3..=4).contains(&arguments.len()) {
            return Err(self.arity("find-method", "three or four", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let qualifiers = self.method_qualifiers_from_value(&arguments[1], span)?;
        let specializers = self.method_specializers_from_value(&arguments[2], span)?;
        let errorp = arguments.get(3).is_none_or(Value::is_truthy);
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };

        let methods = methods.borrow();
        let method = methods.iter().find(|method| {
            method.qualifiers == qualifiers
                && method.specializers.len() == specializers.len()
                && method
                    .specializers
                    .iter()
                    .zip(specializers.iter())
                    .all(|(left, right)| self.same_method_specializer(left, right))
        });
        match method {
            Some(method) => Ok(Value::Method(Rc::new(method.clone()))),
            None if errorp => Err(self.invalid("method not found", span)),
            None => Ok(Value::Nil),
        }
    }

    fn method_qualifiers_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<String>, RuntimeError> {
        let items = value
            .list_items()
            .ok_or_else(|| self.invalid("find-method qualifiers must be a proper list", span))?;
        items
            .iter()
            .map(|item| {
                let (name, _) = item.symbol_reference().ok_or_else(|| RuntimeError::Type {
                    expected: "SYMBOL".to_owned(),
                    actual: item.type_name().to_string(),
                    span: Some(span),
                })?;
                Ok(normalize_name(name).trim_start_matches(':').to_owned())
            })
            .collect()
    }

    fn method_specializers_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Vec<MethodSpecializer>, RuntimeError> {
        let items = value
            .list_items()
            .ok_or_else(|| self.invalid("find-method specializers must be a proper list", span))?;
        items
            .iter()
            .map(|item| self.method_specializer_from_value(item, span))
            .collect()
    }

    fn method_specializer_from_value(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<MethodSpecializer, RuntimeError> {
        if let Value::Class(class) = value {
            return Ok(MethodSpecializer::Class(class.name.clone()));
        }
        if let Some((name, exact)) = value.symbol_reference() {
            let class = if exact {
                name.to_owned()
            } else {
                unqualified_name(name)
            };
            return Ok(MethodSpecializer::Class(class));
        }
        if let Some(items) = value.list_items()
            && items.len() == 2
            && items[0]
                .symbol_reference()
                .is_some_and(|(name, _)| normalize_name(name) == "EQL")
        {
            return Ok(MethodSpecializer::Eql(items[1].clone()));
        }
        Err(RuntimeError::Type {
            expected: "CLASS".to_owned(),
            actual: value.type_name().to_string(),
            span: Some(span),
        })
    }

    fn same_method_specializer(&self, left: &MethodSpecializer, right: &MethodSpecializer) -> bool {
        match (left, right) {
            (MethodSpecializer::Class(left), MethodSpecializer::Class(right)) => left == right,
            (MethodSpecializer::Eql(left), MethodSpecializer::Eql(right)) => {
                builtins::eql_value(left, right)
            }
            _ => false,
        }
    }

    fn compute_applicable_methods(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(self.arity("compute-applicable-methods", "two", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let method_arguments = arguments[1].list_items().ok_or_else(|| {
            self.invalid(
                "compute-applicable-methods arguments must be a proper list",
                span,
            )
        })?;
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            self.ordered_applicable_methods(methods, &method_arguments)
                .into_iter()
                .map(|method| Value::Method(Rc::new(method)))
                .collect(),
        ))
    }

    fn generic_function_methods(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-methods", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { methods, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            methods
                .borrow()
                .iter()
                .cloned()
                .map(|method| Value::Method(Rc::new(method)))
                .collect(),
        ))
    }

    fn generic_function_name(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-name", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { name, .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::symbol(name.clone()))
    }

    fn generic_function_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("generic-function-class", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Self::class_object_named(
            "STANDARD-GENERIC-FUNCTION",
            environment,
        ))
    }

    fn method_class(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-class", "one", arguments.len()));
        }
        let Value::Method(_) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Self::class_object_named("STANDARD-METHOD", environment))
    }

    fn method_combination(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-combination", "one", arguments.len()));
        }
        let function = self.resolve_function_designator(&arguments[0], span, environment)?;
        let crate::Function::Generic { .. } = function.as_ref() else {
            return Err(RuntimeError::Type {
                expected: "GENERIC-FUNCTION".to_owned(),
                actual: Value::Function(function).type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::symbol("STANDARD"))
    }

    fn method_qualifiers(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-qualifiers", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            method.qualifiers.iter().map(Value::keyword).collect(),
        ))
    }

    fn method_function(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-function", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(method.function.clone())
    }

    fn method_generic_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-generic-function", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(Value::Function(function)) = environment.lookup_function(&method.generic_function)
        else {
            return Ok(Value::Nil);
        };
        match function.as_ref() {
            crate::Function::Generic { .. } => Ok(Value::Function(function)),
            _ => Ok(Value::Nil),
        }
    }

    fn method_lambda_list(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-lambda-list", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(method.lambda_list.clone())
    }

    fn method_specializers(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("method-specializers", "one", arguments.len()));
        }
        let Value::Method(method) = &arguments[0] else {
            return Err(RuntimeError::Type {
                expected: "METHOD".to_owned(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        };
        Ok(Value::list(
            method
                .specializers
                .iter()
                .map(|specializer| self.method_specializer_value(specializer, environment))
                .collect(),
        ))
    }

    fn method_specializer_value(
        &self,
        specializer: &MethodSpecializer,
        environment: &Environment,
    ) -> Value {
        match specializer {
            MethodSpecializer::Class(class_name) => environment
                .lookup_class(class_name)
                .map(Value::class_object)
                .unwrap_or_else(|| {
                    Value::class_object(Rc::new(ClassDefinition {
                        name: class_name.clone(),
                        precedence: vec![class_name.clone(), "STANDARD-OBJECT".to_owned()],
                        slots: Vec::new(),
                        default_initargs: Vec::new(),
                        documentation: Rc::new(RefCell::new(None)),
                    }))
                }),
            MethodSpecializer::Eql(value) => Value::list(vec![Value::symbol("EQL"), value.clone()]),
        }
    }

    fn class_object_named(name: &str, environment: &Environment) -> Value {
        environment
            .lookup_class(name)
            .map(Value::class_object)
            .unwrap_or_else(|| {
                Value::class_object(Rc::new(ClassDefinition {
                    name: name.to_owned(),
                    precedence: vec![name.to_owned(), "STANDARD-OBJECT".to_owned()],
                    slots: Vec::new(),
                    default_initargs: Vec::new(),
                    documentation: Rc::new(RefCell::new(None)),
                }))
            })
    }

    fn special_defmethod(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defmethod", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defmethod name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_index = items[2..]
            .iter()
            .position(|form| matches!(form.kind, FormKind::List(_)))
            .map(|index| index + 2)
            .ok_or_else(|| {
                self.invalid("defmethod requires a method lambda list", items[1].span)
            })?;

        let qualifiers = items[2..lambda_index]
            .iter()
            .map(|form| {
                let qualifier = self.definition_name_from_form(form, "defmethod qualifier")?;
                match qualifier.as_str() {
                    "BEFORE" | "AFTER" | "AROUND" => Ok(qualifier),
                    _ => Err(self.invalid("unsupported defmethod qualifier", form.span)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let FormKind::List(parameters) = &items[lambda_index].kind else {
            return Err(self.invalid(
                "defmethod lambda list must be a list",
                items[lambda_index].span,
            ));
        };

        let mut required = Vec::new();
        let mut required_escaped = Vec::new();
        let mut specializers = Vec::new();
        let mut normalized_parameters = Vec::new();
        let mut required_parameter_count = 0;
        for parameter in parameters {
            if matches!(&parameter.kind, FormKind::Atom(name) if normalize_name(name).starts_with('&'))
            {
                break;
            }
            let (name_form, specializer_form) = match &parameter.kind {
                FormKind::Atom(_) => (parameter, None),
                FormKind::List(parts) if (1..=2).contains(&parts.len()) => {
                    (&parts[0], parts.get(1))
                }
                _ => {
                    return Err(self.invalid(
                        "defmethod parameter must be a variable or (variable class)",
                        parameter.span,
                    ));
                }
            };
            let (parameter_name, escaped) =
                self.variable_name_info(name_form, "defmethod parameter must be a variable")?;
            required.push(unqualified_name(&parameter_name));
            required_escaped.push(escaped);
            let specializer = match specializer_form {
                None => MethodSpecializer::Class("T".to_owned()),
                Some(form) => {
                    let is_eql = matches!(&form.kind, FormKind::List(parts) if parts.len() == 2
                        && atom_name(&parts[0]).is_some_and(|name| normalize_name(name) == "EQL"));
                    if is_eql {
                        let FormKind::List(parts) = &form.kind else {
                            unreachable!();
                        };
                        MethodSpecializer::Eql(self.eval_in(&parts[1], environment)?)
                    } else {
                        let class =
                            self.definition_name_from_form(form, "defmethod specializer")?;
                        if class != "T"
                            && class != "OBJECT"
                            && class != "STANDARD-OBJECT"
                            && environment.lookup_class(&class).is_none()
                        {
                            return Err(
                                self.invalid("unknown defmethod specializer", parameter.span)
                            );
                        }
                        MethodSpecializer::Class(class)
                    }
                }
            };
            specializers.push(specializer);
            normalized_parameters.push(name_form.clone());
            required_parameter_count += 1;
        }
        normalized_parameters.extend(
            parameters
                .get(required_parameter_count..)
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
        let normalized_lambda_list = Form::list(normalized_parameters, items[lambda_index].span);
        let lambda_list = self.parameters(&normalized_lambda_list)?;

        let generic = environment.lookup_function(&name).or_else(|| {
            let generic = Value::generic(name.clone(), lambda_list.clone());
            environment.define_function(&name, generic.clone());
            Some(generic)
        });
        let Some(Value::Function(generic)) = generic else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        let crate::Function::Generic {
            methods,
            lambda_list: generic_lambda_list,
            ..
        } = generic.as_ref()
        else {
            return Err(self.invalid("defmethod name is not a generic function", items[1].span));
        };
        self.ensure_generic_lambda_list_congruence(
            generic_lambda_list,
            &lambda_list,
            items[lambda_index].span,
        )?;
        let closure = Value::closure_with_keywords(ClosureData {
            parameters: required,
            required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[lambda_index + 1..].to_vec(),
            environment: environment.clone(),
        });
        let definition = MethodDefinition {
            id: self.fresh_method_id(),
            generic_function: name.clone(),
            lambda_list: self.quoted_value(&normalized_lambda_list)?,
            qualifiers,
            specializers,
            function: closure,
        };
        let mut methods = methods.borrow_mut();
        if let Some(existing) = methods
            .iter_mut()
            .find(|method| self.same_method_identity(method, &definition))
        {
            *existing = definition;
        } else {
            methods.push(definition);
        }
        Ok(Value::symbol(name))
    }

    fn ensure_generic_lambda_list_congruence(
        &self,
        generic: &OrdinaryLambdaList,
        method: &OrdinaryLambdaList,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if generic.required.len() != method.required.len() {
            return Err(self.invalid(
                "defmethod lambda list is not congruent with generic function",
                span,
            ));
        }
        if !generic.optional.is_empty() && generic.optional.len() != method.optional.len() {
            return Err(self.invalid(
                "defmethod lambda list is not congruent with generic function",
                span,
            ));
        }
        if generic.has_keyword_section {
            if !method.has_keyword_section {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
            if generic.allow_other_keys && !method.allow_other_keys {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
            let method_keywords = method
                .keywords
                .iter()
                .map(|parameter| normalize_name(&parameter.keyword_name))
                .collect::<HashSet<_>>();
            if generic
                .keywords
                .iter()
                .map(|parameter| normalize_name(&parameter.keyword_name))
                .any(|keyword| !method_keywords.contains(&keyword))
            {
                return Err(self.invalid(
                    "defmethod lambda list is not congruent with generic function",
                    span,
                ));
            }
        }
        Ok(())
    }

}

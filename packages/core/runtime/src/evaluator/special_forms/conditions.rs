impl Runtime {
    fn special_values(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let values = items[1..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::values(values))
    }

    fn special_multiple_value_list(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("multiple-value-list", "one", items.len().saturating_sub(1)));
        }
        let values = self
            .eval_values_in(&items[1], environment)?
            .multiple_values();
        Ok(Value::list(values))
    }

    fn special_ignore_errors(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match self.eval_sequence_values(&items[1..], environment) {
            Ok(value) => Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => Err(error),
            Err(error @ RuntimeError::Go { .. }) => Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => Err(error),
            Err(error) => Ok(Value::values(vec![Value::Nil, Value::condition(&error)])),
        }
    }

    fn special_handler_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "handler-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        let mut handlers = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(self.invalid("handler-case clause must be a list", clause.span));
            };
            if clause_items.len() < 2 {
                return Err(self.invalid(
                    "handler-case clause needs a condition and body",
                    clause.span,
                ));
            }
            let FormKind::List(variables) = &clause_items[1].kind else {
                return Err(self.invalid(
                    "handler-case variable list must be a list",
                    clause_items[1].span,
                ));
            };
            if variables.len() > 1 {
                return Err(self.invalid(
                    "handler-case accepts at most one condition variable",
                    clause_items[1].span,
                ));
            }
            let condition = self.condition_name(&clause_items[0])?;
            if let Some(variable) = variables.first() {
                self.variable_name_info(variable, "handler-case condition variable")?;
            }
            handlers.push(ConditionHandlerBinding {
                condition,
                function: None,
                catch: true,
            });
        }

        let guard = self.condition_handler_guard(handlers);
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        let protected = match protected_result {
            Ok(value) => return Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
            Err(error @ RuntimeError::Go { .. }) => return Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
            Err(error) => error,
        };

        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                unreachable!("handler-case clauses were validated above");
            };
            let condition = self.condition_name(&clause_items[0])?;
            if !protected.matches_condition(&condition) {
                continue;
            }
            let local = environment.child();
            if let FormKind::List(variables) = &clause_items[1].kind
                && let Some(variable) = variables.first()
            {
                let (name, escaped) =
                    self.variable_name_info(variable, "handler-case condition variable")?;
                self.define_variable_in(&name, escaped, Value::condition(&protected), &local);
            }
            return self.eval_sequence_values(&clause_items[2..], &local);
        }

        Err(protected)
    }

    fn special_handler_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("handler-bind", "at least one", 0));
        }
        let FormKind::List(handlers) = &items[1].kind else {
            return Err(self.invalid("handler-bind handler list must be a list", items[1].span));
        };
        let mut handler_bindings = Vec::with_capacity(handlers.len());
        for handler in handlers {
            let FormKind::List(parts) = &handler.kind else {
                return Err(self.invalid("handler-bind clause must be a list", handler.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "handler-bind clause needs a condition and function",
                    handler.span,
                ));
            }
            let condition = self.condition_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            handler_bindings.push(ConditionHandlerBinding {
                condition,
                function: Some(function),
                catch: false,
            });
        }

        let guard = self.condition_handler_guard(handler_bindings.clone());
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        let body = match body_result {
            Ok(value) => return Ok(value),
            Err(error @ RuntimeError::ReturnFrom { .. }) => return Err(error),
            Err(error @ RuntimeError::Go { .. }) => return Err(error),
            Err(error @ RuntimeError::InvokeRestart { .. }) => return Err(error),
            Err(error @ RuntimeError::Signaled { .. }) => return Err(error),
            Err(error) => error,
        };

        for (handler, binding) in handlers.iter().zip(handler_bindings.iter()).rev() {
            let FormKind::List(parts) = &handler.kind else {
                unreachable!("handler-bind clauses were validated above");
            };
            if body.matches_condition(&binding.condition) {
                let Some(function) = &binding.function else {
                    return Err(body);
                };
                return self.apply_in(
                    function,
                    &[Value::condition(&body)],
                    parts[1].span,
                    environment,
                );
            }
        }

        Err(body)
    }

    fn special_restart_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("restart-bind", "at least one", 0));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("restart-bind binding list must be a list", items[1].span));
        };

        let mut restarts = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("restart-bind clause must be a list", binding.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "restart-bind clause needs a name and function",
                    binding.span,
                ));
            }
            let name = self.restart_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            restarts.push((name, function, parts[1].span));
        }

        let guard = self.restart_guard(
            restarts
                .iter()
                .map(|(name, function, _)| {
                    RestartBinding::new(name.clone(), Some(function.clone()))
                })
                .collect(),
        );
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(error) => {
                let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                else {
                    return Err(error);
                };
                let Some((_, function, binding_span)) = restarts
                    .iter()
                    .rev()
                    .find(|(name, _, _)| normalize_name(invoked.as_str()) == name.as_str())
                else {
                    return Err(error);
                };
                let argument_values = arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect::<Vec<_>>();
                self.apply_in(function, &argument_values, *binding_span, environment)
            }
        }
    }

    fn special_catch(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("catch", "at least one", 0));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        match self.eval_sequence_values(&items[2..], environment) {
            Ok(value) => Ok(value),
            Err(RuntimeError::Throw {
                tag: thrown_tag,
                value,
                ..
            }) if thrown_tag.matches(&tag) => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_with_simple_restart(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "with-simple-restart",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(clause) = &items[1].kind else {
            return Err(self.invalid(
                "with-simple-restart restart clause must be a list",
                items[1].span,
            ));
        };
        if clause.len() < 2 {
            return Err(self.invalid(
                "with-simple-restart restart clause needs a name and report format",
                items[1].span,
            ));
        }
        let name = self.restart_name(&clause[0])?;
        let guard = self.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(RuntimeError::InvokeRestart {
                name: invoked,
                value,
                ..
            }) if normalize_name(invoked.as_str()) == name => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_with_condition_restarts(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.arity(
                "with-condition-restarts",
                "at least three",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_values_in(&items[1], environment)?.primary_value();
        if condition.condition_type_name().is_none() {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_string(),
                actual: condition.type_name().to_string(),
                span: Some(items[1].span),
            });
        }
        let restarts_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let Some(restarts) = restarts_value.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: restarts_value.type_name().to_string(),
                span: Some(items[2].span),
            });
        };
        if let Some(restart) = restarts
            .iter()
            .find(|restart| restart.restart_name().is_none())
        {
            return Err(RuntimeError::Type {
                expected: "RESTART".to_string(),
                actual: restart.type_name().to_string(),
                span: Some(items[2].span),
            });
        }
        let guard = self.condition_restart_guard(condition, restarts);
        let result = self.eval_sequence_values(&items[3..], environment);
        drop(guard);
        result
    }

    fn special_restart_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "restart-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("restart-case clause must be a list", clause.span));
            };
            if parts.len() < 2 {
                return Err(self.invalid(
                    "restart-case clause needs a name, lambda list, and body",
                    clause.span,
                ));
            }
            self.restart_name(&parts[0])?;
            self.parameters(&parts[1])?;
        }

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                unreachable!("restart-case clauses were validated above");
            };
            let name = self.restart_name(&parts[0])?;
            let lambda_list = self.parameters(&parts[1])?;
            let closure = Value::closure_with_keywords(ClosureData {
                parameters: lambda_list.required.clone(),
                required_escaped: lambda_list.required_escaped.clone(),
                optional: lambda_list.optional.clone(),
                rest: lambda_list.rest.clone(),
                rest_escaped: lambda_list.rest_escaped,
                keywords: lambda_list.keywords.clone(),
                has_keyword_section: lambda_list.has_keyword_section,
                allow_other_keys: lambda_list.allow_other_keys,
                auxiliary: lambda_list.auxiliary.clone(),
                body: parts[2..].to_vec(),
                environment: environment.clone(),
            });
            clauses.push((name, closure, clause.span));
        }

        let guard = self.restart_guard(
            clauses
                .iter()
                .map(|(name, _, _)| RestartBinding::new(name.clone(), None))
                .collect(),
        );
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        match protected_result {
            Ok(value) => Ok(value),
            Err(error) => {
                if let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                    && let Some((_, closure, clause_span)) =
                        clauses.iter().find(|(restart, _, _)| {
                            normalize_name(invoked.as_str()) == restart.as_str()
                        })
                {
                    let argument_values = arguments
                        .iter()
                        .cloned()
                        .map(ReturnValue::into_value)
                        .collect::<Vec<_>>();
                    return self.apply_in(closure, &argument_values, *clause_span, environment);
                }
                Err(error)
            }
        }
    }

    fn special_throw(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("throw", "two", items.len().saturating_sub(1)));
        }

        let tag = self.eval_values_in(&items[1], environment)?.primary_value();
        let value = self.eval_values_in(&items[2], environment)?;
        Err(RuntimeError::Throw {
            tag: ThrowTag::new(tag),
            value: ReturnValue::new(value),
            span: Some(items[0].span),
        })
    }

    fn special_progv(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("progv", "at least two", items.len().saturating_sub(1)));
        }

        let symbols_value = self.eval_values_in(&items[1], environment)?.primary_value();
        let symbols = symbols_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: symbols_value.type_name().to_string(),
                span: Some(items[1].span),
            })?;
        let values_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let values = values_value
            .list_items()
            .ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: values_value.type_name().to_string(),
                span: Some(items[2].span),
            })?;

        let _dynamic_guard = self.dynamic_guard();
        for (index, symbol) in symbols.iter().enumerate() {
            let name = symbol.symbol_name().ok_or_else(|| {
                self.invalid("progv symbol list must contain only symbols", items[1].span)
            })?;
            self.define_dynamic(name, values.get(index).cloned().unwrap_or(Value::Nil));
        }

        self.eval_sequence_values(&items[3..], environment)
    }

    fn special_unwind_protect(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "unwind-protect",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }

        let protected = self.eval_values_in(&items[1], environment);
        let cleanup = self.eval_sequence_values(&items[2..], environment);
        match cleanup {
            Ok(_) => protected,
            Err(error) => Err(error),
        }
    }

    fn special_block(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("block", "at least one", items.len().saturating_sub(1)));
        }
        let name = self.block_name(&items[1])?;
        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block(&name, target);
        match self.eval_sequence_values(&items[2..], &block_environment) {
            Ok(value) => Ok(value),
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

    fn special_return_from(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity("return-from", "one or two", items.len().saturating_sub(1)));
        }
        let block = self.block_name(&items[1])?;
        let value = items.get(2).map_or(Ok(Value::Nil), |form| {
            self.eval_values_in(form, environment)
        })?;
        let target = environment.lookup_block(&block);
        Err(RuntimeError::ReturnFrom {
            block,
            target,
            value: ReturnValue::new(value),
            span: Some(items[1].span),
        })
    }

    fn special_return(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 1 || items.len() == 2) {
            return Err(self.arity("return", "zero or one", items.len().saturating_sub(1)));
        }
        let value = items.get(1).map_or(Ok(Value::Nil), |form| {
            self.eval_values_in(form, environment)
        })?;
        let block = "NIL".to_string();
        let target = environment.lookup_block(&block);
        Err(RuntimeError::ReturnFrom {
            block,
            target,
            value: ReturnValue::new(value),
            span: Some(items[0].span),
        })
    }

    fn special_tagbody(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_tagbody_forms(&items[1..], environment)
    }

    fn eval_tagbody_forms(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut tags: Vec<(String, usize)> = Vec::new();
        for (position, item) in forms.iter().enumerate() {
            if let Some(tag) = control_tag(item) {
                if tags.iter().any(|(known_tag, _)| known_tag == &tag) {
                    return Err(self.invalid("tagbody contains duplicate tag", item.span));
                }
                tags.push((tag, position));
            }
        }

        let target = self.fresh_block_target();
        let tag_environment = environment.child();
        for (tag, _) in &tags {
            tag_environment.define_tag(tag, target);
        }

        let mut position = 0;
        while position < forms.len() {
            let item = &forms[position];
            if control_tag(item).is_some() {
                position += 1;
                continue;
            }
            match self.eval_values_in(item, &tag_environment) {
                Ok(_) => position += 1,
                Err(RuntimeError::Go {
                    tag,
                    target: Some(go_target),
                    ..
                }) if go_target == target => {
                    position = tags
                        .iter()
                        .find(|(known_tag, _)| known_tag == &tag)
                        .map(|(_, tag_position)| *tag_position)
                        .ok_or_else(|| {
                            self.invalid("GO target is missing from TAGBODY", item.span)
                        })?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Value::Nil)
    }

    fn special_go(&self, items: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("go", "one", items.len().saturating_sub(1)));
        }
        let tag = control_tag(&items[1])
            .ok_or_else(|| self.invalid("go tag must be a symbol or integer", items[1].span))?;
        Err(RuntimeError::Go {
            target: environment.lookup_tag(&tag),
            tag,
            span: Some(items[1].span),
        })
    }

    fn special_multiple_value_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "multiple-value-bind",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(self.invalid(
                "multiple-value-bind variables must be a list",
                items[1].span,
            ));
        };
        let variables = variable_forms
            .iter()
            .map(|form| {
                self.variable_name_info(form, "multiple-value-bind variable must be a symbol")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        for (index, (variable, escaped)) in variables.iter().enumerate() {
            self.define_variable_in(
                variable,
                *escaped,
                values.get(index).cloned().unwrap_or(Value::Nil),
                &local,
            );
        }
        self.eval_sequence_values(&items[3..], &local)
    }

    fn special_multiple_value_call(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "multiple-value-call",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let function = self.eval_in(&items[1], environment)?;
        let mut arguments = Vec::new();
        for form in &items[2..] {
            arguments.extend(self.eval_values_in(form, environment)?.multiple_values());
        }
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    fn special_multiple_value_prog1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "multiple-value-prog1",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let result = self.eval_values_in(&items[1], environment)?;
        self.eval_sequence_values(&items[2..], environment)?;
        Ok(result)
    }

}

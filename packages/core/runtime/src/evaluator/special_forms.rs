macro_rules! evaluator_special_forms {
    () => {
    fn special_quote(&self, items: &[Form], span: Span) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("quote", "one", items.len().saturating_sub(1)));
        }
        self.quoted_value(&items[1]).map_err(|error| match error {
            RuntimeError::InvalidForm { .. } => self.invalid("invalid quoted form", span),
            error => error,
        })
    }

    fn special_the(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("the", "two", items.len().saturating_sub(1)));
        }
        let type_designator = quoted_form_value(&items[1])?;
        let value = self.eval_in(&items[2], environment)?;
        builtins::the_check_in(&[value, type_designator], environment)
    }

    fn special_load_time_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "load-time-value",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_values_in(&items[1], environment)?;
        if let Some(read_only_p) = items.get(2) {
            let _ = self.eval_in(read_only_p, environment)?;
        }
        Ok(value)
    }

    fn special_nth_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("nth-value", "two", items.len().saturating_sub(1)));
        }

        let index_value = self.eval_in(&items[1], environment)?;
        let index = match index_value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
            }
            Value::Integer(_) => {
                return Err(self.invalid("nth-value index must be non-negative", items[1].span));
            }
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(items[1].span),
                });
            }
        };

        let values = self
            .eval_values_in(&items[2], environment)?
            .multiple_values();
        Ok(values.get(index).cloned().unwrap_or(Value::Nil))
    }

    fn special_locally(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(items.get(1..).unwrap_or(&[]), environment)
    }

    fn special_eval_when(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("eval-when", "at least one", items.len().saturating_sub(1)));
        }
        if self.eval_when_executes(&items[1])? {
            self.eval_sequence_values(items.get(2..).unwrap_or(&[]), environment)
        } else {
            Ok(Value::Nil)
        }
    }

    fn eval_when_executes(&self, form: &Form) -> Result<bool, RuntimeError> {
        let FormKind::List(situations) = &form.kind else {
            return Err(self.invalid("eval-when situations must be a list", form.span));
        };
        let mut executes = false;
        for situation in situations {
            let Some(name) = atom_name(situation) else {
                return Err(
                    self.invalid("eval-when situations must contain symbols", situation.span)
                );
            };
            let token = parse_symbol_token(name).map_err(|_| {
                self.invalid("eval-when situations must contain symbols", situation.span)
            })?;
            if token.kind == SymbolTokenKind::Uninterned
                || (token.kind == SymbolTokenKind::Symbol && literal_atom(name).is_some())
            {
                return Err(
                    self.invalid("eval-when situations must contain symbols", situation.span)
                );
            }
            if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
                executes = true;
            }
        }
        Ok(executes)
    }

    fn special_quasiquote(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("quasiquote", "one", items.len().saturating_sub(1)));
        }
        self.quasiquote_value(&items[1], environment)
    }

    pub(crate) fn quasiquote_value(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.quasiquote_value_at(form, environment, 1)
    }

    fn quasiquote_value_at(
        &self,
        form: &Form,
        environment: &Environment,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) | FormKind::String(_) | FormKind::Character(_) => {
                self.quoted_value(form)
            }
            FormKind::Vector(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1
                        && let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        )
                    {
                        values.extend(self.quasiquote_splice(argument, environment, item.span)?);
                        continue;
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                Ok(Value::vector(values))
            }
            FormKind::List(items) => {
                if let Some(argument) = prefix_argument(items, "UNQUOTE") {
                    if depth == 1 {
                        return self.eval_in(argument, environment);
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE",
                        self.quasiquote_value_at(argument, environment, depth - 1)?,
                    ));
                }
                if let Some(item) = prefix_argument(items, "UNQUOTE-SPLICING") {
                    if depth == 1 {
                        return Err(self.invalid(
                            "unquote-splicing is only valid inside a list or vector",
                            item.span,
                        ));
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE-SPLICING",
                        self.quasiquote_value_at(item, environment, depth - 1)?,
                    ));
                }
                if let Some(argument) = prefix_argument(items, "QUASIQUOTE") {
                    return Ok(quasiquote_marker(
                        "QUASIQUOTE",
                        self.quasiquote_value_at(argument, environment, depth + 1)?,
                    ));
                }

                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1 {
                        if let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        ) {
                            values.extend(self.quasiquote_splice(
                                argument,
                                environment,
                                item.span,
                            )?);
                            continue;
                        }
                    } else {
                        values.push(self.quasiquote_value_at(item, environment, depth)?);
                        continue;
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                Ok(Value::list(values))
            }
            FormKind::DottedList { items, tail } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    if depth == 1 {
                        if let Some(argument) = prefix_argument(
                            match &item.kind {
                                FormKind::List(items) => items,
                                _ => &[],
                            },
                            "UNQUOTE-SPLICING",
                        ) {
                            values.extend(self.quasiquote_splice(
                                argument,
                                environment,
                                item.span,
                            )?);
                            continue;
                        }
                    } else {
                        values.push(self.quasiquote_value_at(item, environment, depth)?);
                        continue;
                    }
                    values.push(self.quasiquote_value_at(item, environment, depth)?);
                }
                if let Some(argument) = prefix_argument(
                    match &tail.kind {
                        FormKind::List(items) => items,
                        _ => &[],
                    },
                    "UNQUOTE-SPLICING",
                ) && depth == 1
                {
                    let mut spliced = self.quasiquote_splice(argument, environment, tail.span)?;
                    values.append(&mut spliced);
                    return Ok(Value::list(values));
                }
                let tail_value = self.quasiquote_value_at(tail, environment, depth)?;
                if depth == 1
                    && let Some(mut tail_items) = tail_value.list_items()
                {
                    values.append(&mut tail_items);
                    return Ok(Value::list(values));
                }
                Ok(Value::dotted_list(values, tail_value))
            }
        }
    }

    fn quasiquote_splice(
        &self,
        argument: &Form,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = self.eval_in(argument, environment)?;
        value
            .list_items()
            .ok_or_else(|| self.invalid("unquote-splicing requires a proper list", span))
    }

    fn special_if(&self, items: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity("if", "two or three", items.len().saturating_sub(1)));
        }
        let condition = self.eval_in(&items[1], environment)?;
        if condition.is_truthy() {
            self.eval_values_in(&items[2], environment)
        } else {
            items.get(3).map_or(Ok(Value::Nil), |form| {
                self.eval_values_in(form, environment)
            })
        }
    }

    fn special_progn(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(forms, environment)
    }

    fn special_prog1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("prog1", "at least one", items.len().saturating_sub(1)));
        }
        let result = self.eval_values_in(&items[1], environment)?;
        self.eval_sequence_values(&items[2..], environment)?;
        Ok(result)
    }

    fn special_prog2(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("prog2", "at least two", items.len().saturating_sub(1)));
        }
        self.eval_values_in(&items[1], environment)?;
        let result = self.eval_values_in(&items[2], environment)?;
        self.eval_sequence_values(&items[3..], environment)?;
        Ok(result)
    }

    fn special_prog(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "prog*" } else { "prog" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(self.invalid("prog bindings must be a list", items[1].span));
        };

        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) => {
                    if !(1..=2).contains(&parts.len()) {
                        return Err(self.invalid(
                            "prog binding needs a name and optional value",
                            binding.span,
                        ));
                    }
                    let Some(name_form) = parts.first() else {
                        return Err(self.invalid("prog binding needs a name", binding.span));
                    };
                    (name_form, parts.get(1).cloned())
                }
                _ => {
                    return Err(self.invalid("prog binding must be a symbol or list", binding.span));
                }
            };
            let (name, escaped) =
                self.variable_name_info(name_form, "prog binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(self.invalid("prog binding names must be unique", name_form.span));
            }
            bindings.push((name, escaped, init));
        }

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let execute = || -> Result<Value, RuntimeError> {
            if sequential {
                for (name, escaped, init) in &bindings {
                    let value = init
                        .as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, &local))?;
                    self.define_variable_in(name, *escaped, value, &local);
                }
            } else {
                let mut values = Vec::with_capacity(bindings.len());
                for (_, _, init) in &bindings {
                    values.push(init.as_ref().map_or(Ok(Value::Nil), |form| {
                        self.eval_in(form, &block_environment)
                    })?);
                }
                for ((name, escaped, _), value) in bindings.iter().zip(values) {
                    self.define_variable_in(name, *escaped, value, &local);
                }
            }

            self.eval_tagbody_forms(&items[2..], &local)?;
            Ok(Value::Nil)
        };

        match execute() {
            Ok(value) => Ok(value),
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }

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

    fn special_and(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::boolean(true);
        for (index, form) in forms.iter().enumerate() {
            result = self.eval_values_in(form, environment)?;
            if !result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(result)
    }

    fn special_or(&self, forms: &[Form], environment: &Environment) -> Result<Value, RuntimeError> {
        for (index, form) in forms.iter().enumerate() {
            let result = self.eval_values_in(form, environment)?;
            if result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(Value::Nil)
    }

    fn special_when(
        &self,
        items: &[Form],
        environment: &Environment,
        positive: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if positive { "when" } else { "unless" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?.is_truthy();
        if condition == positive {
            self.eval_sequence_values(&items[2..], environment)
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_cond(
        &self,
        clauses: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for clause in clauses {
            let FormKind::List(items) = &clause.kind else {
                return Err(self.invalid("cond clauses must be lists", clause.span));
            };
            if items.is_empty() {
                return Err(self.invalid("cond clause cannot be empty", clause.span));
            }
            let condition = self.eval_in(&items[0], environment)?;
            if condition.is_truthy() {
                return if items.len() == 1 {
                    Ok(condition)
                } else {
                    self.eval_sequence_values(&items[1..], environment)
                };
            }
        }
        Ok(Value::Nil)
    }

    fn special_case(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss { "ecase" } else { "case" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }

        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("case clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("case clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }

            let keys = match &parts[0].kind {
                FormKind::List(keys) => keys.as_slice(),
                _ => std::slice::from_ref(&parts[0]),
            };
            for key_form in keys {
                let candidate = quoted_form_value(key_form)?;
                if builtins::eql_value(&key, &candidate) {
                    return self.eval_sequence_values(&parts[1..], environment);
                }
            }
        }

        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("ecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_typecase(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss {
            "etypecase"
        } else {
            "typecase"
        };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }

        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("typecase clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("typecase clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }

            let type_designator = quoted_form_value(&parts[0])?;
            if builtins::typep_value_in(&key, &type_designator, environment)? {
                return self.eval_sequence_values(&parts[1..], environment);
            }
        }

        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("etypecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    fn special_destructuring_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "destructuring-bind",
                "two or more",
                items.len().saturating_sub(1),
            ));
        }
        let lambda_list = match &items[1].kind {
            FormKind::List(_) => Some(self.macro_parameters(&items[1], true)?),
            _ => None,
        };
        let mut seen = HashSet::new();
        let pattern = lambda_list
            .is_none()
            .then(|| self.macro_pattern(&items[1], &mut seen, true));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = lambda_list {
            self.bind_destructuring_lambda_list(&lambda_list, value, &local, items[1].span)?;
        } else if let Some(pattern) = pattern {
            self.bind_macro_pattern(&pattern, value, &local, items[1].span)?;
        }
        self.eval_sequence_values(&items[3..], &local)
    }

    fn special_let(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if sequential { "let*" } else { "let" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("let bindings must be a list", items[1].span));
        };
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(self.invalid("let binding must be a list", binding.span));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(
                    self.invalid("let binding needs a name and optional value", binding.span)
                );
            }
            let (name, escaped) =
                self.variable_name_info(&binding_items[0], "let binding name must be a symbol")?;
            let value = binding_items.get(1).map_or(Ok(Value::Nil), |form| {
                self.eval_in(form, if sequential { &local } else { environment })
            })?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_flet(
        &self,
        items: &[Form],
        environment: &Environment,
        recursive: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if recursive { "labels" } else { "flet" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local function bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let captured = if recursive {
            local.clone()
        } else {
            environment.clone()
        };
        let mut names = HashSet::new();
        let mut definitions = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("local function binding must be a list", binding.span));
            };
            if parts.len() < 3 {
                return Err(self.invalid(
                    "local function needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (normalized, escaped) =
                self.variable_name_info(&parts[0], "local function name must be a symbol")?;
            if !names.insert(normalized.clone()) {
                return Err(self.invalid("local function names must be unique", parts[0].span));
            }
            definitions.push((
                normalized,
                escaped,
                self.parameters(&parts[1])?,
                parts[2..].to_vec(),
            ));
        }

        for (name, escaped, lambda_list, body) in definitions {
            let function = Value::closure_with_keywords(ClosureData {
                parameters: lambda_list.required,
                required_escaped: lambda_list.required_escaped,
                optional: lambda_list.optional,
                rest: lambda_list.rest,
                rest_escaped: lambda_list.rest_escaped,
                keywords: lambda_list.keywords,
                has_keyword_section: lambda_list.has_keyword_section,
                allow_other_keys: lambda_list.allow_other_keys,
                auxiliary: lambda_list.auxiliary,
                body,
                environment: captured.clone(),
            });
            if escaped {
                local.define_function_exact(name, function);
            } else {
                local.define_function(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("macrolet", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("local macro bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let captured = environment.clone();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("local macro binding must be a list", binding.span));
            };
            if parts.len() < 3 {
                return Err(self.invalid(
                    "local macro needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "local macro name must be a symbol")?;
            if !names.insert(name.clone()) {
                return Err(self.invalid("local macro names must be unique", parts[0].span));
            }
            let lambda_list = self.macro_parameters(&parts[1], false)?;
            let function =
                Value::macro_function(lambda_list, parts[2..].to_vec(), captured.clone());
            if escaped {
                local.define_exact(name, function);
            } else {
                local.define(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_symbol_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                "symbol-macrolet",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid("symbol macro bindings must be a list", items[1].span));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("symbol macro binding must be a list", binding.span));
            };
            if parts.len() != 2 {
                return Err(self.invalid(
                    "symbol macro binding needs a name and an expansion",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "symbol macro name must be a symbol")?;
            if !names.insert((name.clone(), escaped)) {
                return Err(self.invalid("symbol macro names must be unique", parts[0].span));
            }
            if escaped {
                local.define_symbol_macro_exact(name, parts[1].clone());
            } else {
                local.define_symbol_macro(name, parts[1].clone());
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    fn special_define_symbol_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("DEFINE-SYMBOL-MACRO", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) =
            self.variable_name_info(&items[1], "DEFINE-SYMBOL-MACRO name must be a symbol")?;
        if escaped {
            environment.define_symbol_macro_exact(name.clone(), items[2].clone());
        } else {
            environment.define_symbol_macro(name.clone(), items[2].clone());
        }
        Ok(if escaped {
            Value::symbol_exact(name)
        } else {
            Value::symbol(name)
        })
    }

    fn special_dotimes(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("dotimes", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(self.invalid("dotimes binding must be a list", items[1].span));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(self.invalid(
                "dotimes binding needs a name, count, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            self.variable_name_info(&binding[0], "dotimes binding name must be a symbol")?;
        let count_form = &binding[1];
        let count = match self.eval_in(count_form, environment)? {
            Value::Integer(count) => count,
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(count_form.span),
                });
            }
        };

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        self.define_variable_in(&name, escaped, Value::Integer(0), &local);
        let mut index = 0;
        while index < count {
            self.eval_sequence_values(&items[2..], &local)?;
            index += 1;
            self.set_variable_in(&name, escaped, Value::Integer(index), &local);
        }
        binding
            .get(2)
            .map_or(Ok(Value::Nil), |result| self.eval_values_in(result, &local))
    }

    fn special_dolist(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("dolist", "at least one", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(self.invalid("dolist binding must be a list", items[1].span));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(self.invalid(
                "dolist binding needs a name, list, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            self.variable_name_info(&binding[0], "dolist binding name must be a symbol")?;
        let list_form = &binding[1];
        let list = self.eval_in(list_form, environment)?;
        let Some(elements) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(list_form.span),
            });
        };

        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        self.define_variable_in(&name, escaped, Value::Nil, &local);
        for element in elements {
            self.set_variable_in(&name, escaped, element, &local);
            self.eval_sequence_values(&items[2..], &local)?;
        }
        self.set_variable_in(&name, escaped, Value::Nil, &local);
        binding
            .get(2)
            .map_or(Ok(Value::Nil), |result| self.eval_values_in(result, &local))
    }

    fn special_do(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "do*" } else { "do" };
        if items.len() < 3 {
            return Err(self.arity(operator, "at least two", items.len().saturating_sub(1)));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(self.invalid("do bindings must be a list", items[1].span));
        };
        let FormKind::List(termination) = &items[2].kind else {
            return Err(self.invalid("do termination must be a list", items[2].span));
        };
        if termination.is_empty() {
            return Err(self.invalid("do termination needs an end test", items[2].span));
        }

        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(self.invalid("do binding must be a list", binding.span));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(self.invalid(
                    "do binding needs a name, optional init, and optional step",
                    binding.span,
                ));
            }
            let (name, escaped) =
                self.variable_name_info(&parts[0], "do binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(self.invalid("do binding names must be unique", parts[0].span));
            }
            bindings.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let initialization = (|| -> Result<(), RuntimeError> {
            if sequential {
                for (name, escaped, init, _) in &bindings {
                    let value = init
                        .as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, &local))?;
                    self.define_variable_in(name, *escaped, value, &local);
                }
            } else {
                let mut values = Vec::with_capacity(bindings.len());
                for (_, _, init, _) in &bindings {
                    values.push(init.as_ref().map_or(Ok(Value::Nil), |form| {
                        self.eval_in(form, &block_environment)
                    })?);
                }
                for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                    self.define_variable_in(name, *escaped, value, &local);
                }
            }
            Ok(())
        })();
        match initialization {
            Ok(()) => {}
            Err(RuntimeError::ReturnFrom {
                target: Some(return_target),
                value,
                ..
            }) if return_target == target => return Ok(value.into_value()),
            Err(error) => return Err(error),
        }

        loop {
            let iteration = (|| -> Result<Option<Value>, RuntimeError> {
                let test = self.eval_in(&termination[0], &local)?;
                if test.is_truthy() {
                    return Ok(Some(self.eval_sequence_values(&termination[1..], &local)?));
                }

                self.eval_tagbody_forms(&items[3..], &local)?;
                if sequential {
                    for (name, escaped, _, step) in &bindings {
                        if let Some(step) = step {
                            let value = self.eval_in(step, &local)?;
                            self.set_variable_in(name, *escaped, value, &local);
                        }
                    }
                } else {
                    let mut values = Vec::with_capacity(bindings.len());
                    for (_, _, _, step) in &bindings {
                        values.push(match step {
                            Some(step) => Some(self.eval_in(step, &local)?),
                            None => None,
                        });
                    }
                    for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                        if let Some(value) = value {
                            self.set_variable_in(name, *escaped, value, &local);
                        }
                    }
                }
                Ok(None)
            })();

            match iteration {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => {}
                Err(RuntimeError::ReturnFrom {
                    target: Some(return_target),
                    value,
                    ..
                }) if return_target == target => return Ok(value.into_value()),
                Err(error) => return Err(error),
            }
        }
    }

    fn special_lambda(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.invalid(
                "lambda needs parameters and a body",
                items
                    .first()
                    .map(|item| item.span)
                    .unwrap_or(Span::new(0, 0)),
            ));
        }
        let lambda_list = self.parameters(&items[1])?;
        Ok(Value::closure_with_keywords(ClosureData {
            parameters: lambda_list.required,
            required_escaped: lambda_list.required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[2..].to_vec(),
            environment: environment.clone(),
        }))
    }

    fn special_function(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("function", "one", items.len().saturating_sub(1)));
        }
        if let Some(name) = atom_name(&items[1]) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            return function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(items[1].span),
            });
        }
        self.eval_in(&items[1], environment)
    }

    fn special_defun(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid("defun needs a name, parameters, and a body", items[0].span));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defun name must be a symbol", items[1].span));
        };
        let lambda_list = self.parameters(&items[2])?;
        let documentation = match &items[3].kind {
            FormKind::String(value) => Some(value.clone()),
            _ => None,
        };
        let function = Value::closure_with_keywords(ClosureData {
            parameters: lambda_list.required,
            required_escaped: lambda_list.required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[3..].to_vec(),
            environment: environment.clone(),
        });
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
            if let Some(documentation) = documentation {
                environment.define_function_documentation_exact(&resolved_name, documentation);
            }
        } else {
            self.define_in(&resolved_name, function, environment);
            if let Some(documentation) = documentation {
                environment.define_function_documentation(&resolved_name, documentation);
            }
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_defsetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFSETF accessor must be a symbol", items[1].span));
        };
        let (resolved_name, escaped) = resolved_symbol(accessor);

        match items.len() {
            3 => {
                let writer_designator = if let Some(writer) = atom_name(&items[2]) {
                    let (resolved_name, escaped) = resolved_symbol(writer);
                    if escaped {
                        Value::symbol_exact(resolved_name)
                    } else {
                        Value::symbol(resolved_name)
                    }
                } else {
                    self.eval_in(&items[2], environment)?
                };
                let writer = Value::Function(self.resolve_function_designator(
                    &writer_designator,
                    items[2].span,
                    environment,
                )?);
                environment.define_setf_function(unqualified_name(&resolved_name), writer);
            }
            count if count >= 5 => {
                let lambda_list = self.macro_parameters(&items[2], false)?;
                let FormKind::List(store_items) = &items[3].kind else {
                    return Err(self.invalid(
                        "DEFSETF long form store variables must be a list",
                        items[3].span,
                    ));
                };
                if store_items.len() != 1 {
                    return Err(self.invalid(
                        "DEFSETF long form requires exactly one store variable",
                        items[3].span,
                    ));
                }
                let Some(store_variable) = atom_name(&store_items[0]) else {
                    return Err(self.invalid(
                        "DEFSETF long form store variable must be a symbol",
                        store_items[0].span,
                    ));
                };
                let function = Value::long_defsetf_function(
                    lambda_list,
                    store_variable.to_string(),
                    items[4..].to_vec(),
                    environment.clone(),
                );
                environment.define_setf_expander(unqualified_name(&resolved_name), function);
            }
            _ => {
                return Err(self.invalid(
                    "DEFSETF needs an accessor and a writer, or a long-form expander",
                    items[0].span,
                ));
            }
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_setf_expander(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "DEFINE-SETF-EXPANDER needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFINE-SETF-EXPANDER name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        environment.define_setf_expander(unqualified_name(&resolved_name), function);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_get_setf_expansion(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "GET-SETF-EXPANSION",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place_value = self.eval_in(&items[1], environment)?;
        let place = self.form_from_value(&place_value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let expansion = self.get_setf_expansion(&place, &expansion_environment)?;
        self.setf_expansion_value(&expansion, items[0].span)
    }

    fn special_defmacro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "defmacro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defmacro name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_compiler_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "define-compiler-macro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("define-compiler-macro name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            environment.define_compiler_macro_exact(&resolved_name, function);
        } else {
            environment.define_compiler_macro(&resolved_name, function);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_modify_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "define-modify-macro needs a name, parameters, and a function",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("define-modify-macro name must be a symbol", items[1].span));
        };
        let mut lambda_list = self.macro_parameters(&items[2], false)?;
        lambda_list
            .required
            .insert(0, MacroPattern::Name("NCL-MODIFY-MACRO-PLACE".to_owned()));
        let function =
            Value::modify_macro_function(lambda_list, items[3].clone(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_macroexpand_1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand-1", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = match self.expand_macro_once(&form, &expansion_environment)? {
            Some(expanded) => (expanded, true),
            None => (form, false),
        };
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn special_macroexpand(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self.expand_macros_with_flag(form, &expansion_environment)?;
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn macroexpansion_environment(
        &self,
        value: Value,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(self.global.clone()),
            Value::Environment(environment) => Ok(environment),
            _ => Err(self.invalid("macro expansion environment must be an environment", span)),
        }
    }

    fn special_define(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("define", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) = self.variable_name_info(&items[1], "define name must be a symbol")?;
        let value = self.eval_in(&items[2], environment)?;
        self.define_variable_in(&name, escaped, value.clone(), environment);
        Ok(value)
    }

    fn special_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("setq needs variable/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            let (name, escaped) =
                self.variable_name_info(&pair[0], "setq target must be a symbol")?;
            result = self.eval_in(&pair[1], environment)?;
            if let Some(place) = expansion {
                self.set_place(&place, result.clone(), environment)?;
            } else {
                self.set_or_define_variable_in(
                    &name,
                    escaped,
                    result.clone(),
                    environment,
                    pair[0].span,
                )?;
            }
        }
        Ok(result)
    }

    fn special_psetq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("psetq needs variable/value pairs", items[0].span));
        }
        let mut names = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            names.push((
                self.variable_name_info(&pair[0], "psetq target must be a symbol")?,
                expansion,
            ));
        }
        let values = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                self.eval_values_in(&pair[1], environment)
                    .map(|value| value.primary_value())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (((name, escaped), expansion), value) in names.iter().zip(values) {
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(Value::Nil)
    }

    fn special_multiple_value_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("multiple-value-setq", "two", items.len().saturating_sub(1)));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(self.invalid(
                "multiple-value-setq variables must be a list",
                items[1].span,
            ));
        };
        let names = variable_forms
            .iter()
            .map(|form| {
                Ok::<_, RuntimeError>((
                    self.variable_name_info(form, "multiple-value-setq variable must be a symbol")?,
                    self.expand_symbol_macro_form(form, environment)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        for (index, ((name, escaped), expansion)) in names.iter().enumerate() {
            let value = values.get(index).cloned().unwrap_or(Value::Nil);
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(source.primary_value())
    }

    fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("setf needs place/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let value = if Self::setf_place_uses_multiple_values(&pair[0]) {
                self.eval_values_in(&pair[1], environment)?
            } else {
                self.eval_in(&pair[1], environment)?
            };
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }

    fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("psetf needs place/value pairs", items[0].span));
        }

        let mut assignments = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let value = if Self::setf_place_uses_multiple_values(&pair[0]) {
                self.eval_values_in(&pair[1], environment)?
            } else {
                self.eval_in(&pair[1], environment)?
            };
            assignments.push((pair[0].clone(), value));
        }

        for (place, value) in assignments {
            self.set_place(&place, value, environment)?;
        }
        Ok(Value::Nil)
    }


    };
}

impl Runtime {
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

}

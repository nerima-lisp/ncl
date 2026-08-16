impl Runtime {
    fn prepare_compiled_lambda_environment(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let lambda_list = match self.parameters(form) {
            Ok(lambda_list) => lambda_list,
            Err(RuntimeError::InvalidForm { .. }) => return Ok(environment.child()),
            Err(error) => return Err(error),
        };
        let local = environment.child();
        let define = |name: &str, escaped: bool| {
            if escaped {
                local.define_exact(name, Value::Nil);
            } else {
                local.define(name, Value::Nil);
            }
        };

        for (name, escaped) in lambda_list
            .required
            .iter()
            .zip(lambda_list.required_escaped.iter().copied())
        {
            define(name, escaped);
        }
        for parameter in &lambda_list.optional {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        if let Some(name) = &lambda_list.rest {
            define(name, lambda_list.rest_escaped);
        }
        for parameter in &lambda_list.keywords {
            define(&parameter.name, parameter.name_escaped);
            if let Some(name) = &parameter.supplied_p {
                define(name, parameter.supplied_p_escaped.unwrap_or(false));
            }
        }
        for parameter in &lambda_list.auxiliary {
            define(&parameter.name, parameter.name_escaped);
        }
        Ok(local)
    }

    fn prepare_compiled_destructuring_environment(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Environment, RuntimeError> {
        let local = environment.child();
        match &form.kind {
            FormKind::List(_) => {
                let lambda_list = self.macro_parameters(form, true)?;
                self.define_compile_time_destructuring_lambda_list(&lambda_list, &local);
            }
            _ => {
                let mut seen = HashSet::new();
                let pattern = self.macro_pattern(form, &mut seen, true)?;
                self.define_compile_time_destructuring_pattern(&pattern, &local);
            }
        }
        Ok(local)
    }

    fn define_compile_time_destructuring_pattern(
        &self,
        pattern: &MacroPattern,
        environment: &Environment,
    ) {
        match pattern {
            MacroPattern::Name(name) => {
                environment.define(name, Value::Nil);
            }
            MacroPattern::List(patterns) => {
                for pattern in patterns {
                    self.define_compile_time_destructuring_pattern(pattern, environment);
                }
            }
            MacroPattern::LambdaList(lambda_list) => {
                self.define_compile_time_destructuring_lambda_list(lambda_list, environment);
            }
            MacroPattern::Dotted { items, tail } => {
                for pattern in items {
                    self.define_compile_time_destructuring_pattern(pattern, environment);
                }
                self.define_compile_time_destructuring_pattern(tail, environment);
            }
        }
    }

    fn define_compile_time_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        environment: &Environment,
    ) {
        if let Some(name) = &lambda_list.environment {
            environment.define(name, Value::environment(environment.clone()));
        }
        if let Some(name) = &lambda_list.whole {
            environment.define(name, Value::Nil);
        }
        for pattern in &lambda_list.required {
            self.define_compile_time_destructuring_pattern(pattern, environment);
        }
        for parameter in &lambda_list.optional {
            self.define_compile_time_destructuring_pattern(&parameter.pattern, environment);
            if let Some(name) = &parameter.supplied_p {
                environment.define(name, Value::Nil);
            }
        }
        if let Some(name) = &lambda_list.rest {
            environment.define(name, Value::Nil);
        }
        for parameter in &lambda_list.keywords {
            self.define_compile_time_destructuring_pattern(&parameter.pattern, environment);
            if let Some(name) = &parameter.supplied_p {
                environment.define(name, Value::Nil);
            }
        }
        for parameter in &lambda_list.auxiliary {
            environment.define(&parameter.name, Value::Nil);
        }
    }

    fn prepare_compiled_let(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Form, RuntimeError> {
        let Some(binding_form) = items.get(1) else {
            return Ok(form.clone());
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        };

        let local = environment.child();
        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            if parts.is_empty() {
                prepared_bindings.push(binding.clone());
                continue;
            }

            let (name, escaped) =
                self.variable_name_info(&parts[0], "let binding name must be a symbol")?;
            let mut prepared_parts = parts.to_vec();
            if parts.len() > 1 {
                let initializer_environment = if sequential { &local } else { environment };
                prepared_parts[1] =
                    self.prepare_compiled_form(&parts[1], initializer_environment)?;
            }
            let binding_value = prepared_parts
                .get(1)
                .and_then(|initializer| self.compile_time_binding_value(initializer));
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
            if escaped {
                local.define_exact(name, binding_value.unwrap_or(Value::Nil));
            } else {
                local.define(name, binding_value.unwrap_or(Value::Nil));
            }
        }

        let mut prepared = items.to_vec();
        prepared[1] = Form::list(prepared_bindings, binding_form.span);
        self.prepare_tail(&mut prepared, 2, &local)?;
        Ok(Form::list(prepared, form.span))
    }

    fn compile_time_binding_value(&self, form: &Form) -> Option<Value> {
        if let FormKind::List(items) = &form.kind {
            if is_operator_form(form, "QUOTE") && items.len() == 2 {
                return self.quoted_value(&items[1]).ok();
            }
            return None;
        }

        match &form.kind {
            FormKind::Atom(atom) if literal_atom(atom).is_some() => self.quoted_value(form).ok(),
            FormKind::String(_) | FormKind::Character(_) => self.quoted_value(form).ok(),
            _ => None,
        }
    }

    fn prepare_compiled_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .chunks_exact(2)
            .map(|pair| self.expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut transformed = vec![Form::atom("PROGN", form.span)];
        for (pair, expansion) in items[1..].chunks_exact(2).zip(expansions) {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            let assignment = Form::list(
                vec![Form::atom(operator, pair[0].span), target, pair[1].clone()],
                pair[0].span,
            );
            transformed.push(self.prepare_compiled_form(&assignment, environment)?);
        }
        Ok(Form::list(transformed, form.span))
    }

    fn prepare_compiled_psetq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        }

        let expansions = items[1..]
            .chunks_exact(2)
            .map(|pair| self.expand_symbol_macro_form(&pair[0], environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            for index in (2..prepared.len()).step_by(2) {
                prepared[index] = self.prepare_compiled_form(&items[index], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let mut bindings = Vec::with_capacity(expansions.len());
        let mut body = vec![Form::atom("PROGN", form.span)];
        for (index, (pair, expansion)) in items[1..].chunks_exact(2).zip(expansions).enumerate() {
            let temporary = self.symbol_macro_temporary(form, index, pair[0].span);
            bindings.push(Form::list(
                vec![temporary.clone(), pair[1].clone()],
                pair[0].span,
            ));
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| pair[0].clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![Form::atom(operator, pair[0].span), target, temporary],
                pair[0].span,
            ));
        }
        body.push(Form::atom("NIL", form.span));

        let mut transformed = vec![
            Form::atom("LET", form.span),
            Form::list(bindings, form.span),
        ];
        transformed.push(Form::list(body, form.span));
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }

    fn prepare_compiled_multiple_value_setq(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(variable_form) = items.get(1) else {
            let mut prepared = items.to_vec();
            self.prepare_tail(&mut prepared, 2, environment)?;
            return Ok(Form::list(prepared, form.span));
        };
        let FormKind::List(variable_forms) = &variable_form.kind else {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        };

        let expansions = variable_forms
            .iter()
            .map(|variable| self.expand_symbol_macro_form(variable, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if !expansions.iter().any(|expansion| expansion.is_some()) {
            let mut prepared = items.to_vec();
            if prepared.len() > 2 {
                prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
            }
            return Ok(Form::list(prepared, form.span));
        }

        let temporaries = variable_forms
            .iter()
            .enumerate()
            .map(|(index, variable)| self.symbol_macro_temporary(form, index, variable.span))
            .collect::<Vec<_>>();
        let mut body = Vec::with_capacity(variable_forms.len() + 1);
        for ((variable, expansion), temporary) in variable_forms
            .iter()
            .zip(expansions)
            .zip(temporaries.iter())
        {
            let is_symbol_macro = expansion.is_some();
            let target = expansion.unwrap_or_else(|| variable.clone());
            let operator = if is_symbol_macro { "SETF" } else { "SETQ" };
            body.push(Form::list(
                vec![
                    Form::atom(operator, variable.span),
                    target,
                    temporary.clone(),
                ],
                variable.span,
            ));
        }
        body.push(temporaries[0].clone());

        let mut transformed = vec![
            Form::atom("MULTIPLE-VALUE-BIND", form.span),
            Form::list(temporaries, variable_form.span),
            items[2].clone(),
        ];
        transformed.extend(body);
        self.prepare_compiled_form(&Form::list(transformed, form.span), environment)
    }

    fn symbol_macro_temporary(&self, form: &Form, index: usize, span: Span) -> Form {
        Form::atom(
            format!(
                "NCL-SYMBOL-MACRO-TEMP-{}-{}-{}",
                form.span.start, form.span.end, index
            ),
            span,
        )
    }

    fn prepare_compiled_lambda_list(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared = parameters.to_vec();
        let mut default_section = false;
        for (index, parameter) in parameters.iter().enumerate() {
            if let Some(name) = atom_name(parameter) {
                match normalize_name(name).as_str() {
                    "&OPTIONAL" | "&KEY" | "&AUX" => default_section = true,
                    "&REST" => default_section = false,
                    _ => {}
                }
                continue;
            }
            if !default_section {
                continue;
            }
            let FormKind::List(specification) = &parameter.kind else {
                continue;
            };
            if let Some(default) = specification.get(1) {
                let mut prepared_specification = specification.to_vec();
                prepared_specification[1] = self.prepare_compiled_form(default, environment)?;
                prepared[index] = Form::list(prepared_specification, parameter.span);
            }
        }
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_local_function_bindings(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(bindings) = &form.kind else {
            return Ok(form.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };
            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                let parameter_form = parts[1].clone();
                let local =
                    self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                prepared_parts[1] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] = self.prepare_compiled_form(&parts[index], &local)?;
                }
            } else {
                for index in 2..prepared_parts.len() {
                    prepared_parts[index] =
                        self.prepare_compiled_form(&parts[index], environment)?;
                }
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }
        Ok(Form::list(prepared_bindings, form.span))
    }


}
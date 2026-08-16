macro_rules! evaluator_compiler {
    () => {
    fn prepare_compiled_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(form, environment)? {
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "MACROLET") {
            return self.prepare_compiled_macrolet(form, environment);
        }
        if is_operator_form(form, "SYMBOL-MACROLET") {
            return self.prepare_compiled_symbol_macrolet(form, environment);
        }
        if is_operator_form(form, "WITH-OPEN-FILE") {
            let expanded = self.expand_with_open_file(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-OUTPUT-TO-STRING") {
            let expanded = self.expand_with_output_to_string(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if is_operator_form(form, "WITH-INPUT-FROM-STRING") {
            let expanded = self.expand_with_input_from_string(form)?;
            return self.prepare_compiled_form(&expanded, environment);
        }
        if let Some(expanded) = self.expand_compiler_macro_once(form, environment)? {
            return self.prepare_compiled_form(&expanded, environment);
        }

        if is_operator_form(form, "DEFMACRO")
            || is_operator_form(form, "DEFINE-COMPILER-MACRO")
            || is_operator_form(form, "DEFINE-MODIFY-MACRO")
            || is_operator_form(form, "DEFINE-SETF-EXPANDER")
            || is_operator_form(form, "DEFINE-SYMBOL-MACRO")
            || is_operator_form(form, "MACROEXPAND-1")
            || is_operator_form(form, "MACROEXPAND")
            || is_operator_form(form, "LOAD-TIME-VALUE")
            || is_operator_form(form, "DEFPACKAGE")
            || is_operator_form(form, "IN-PACKAGE")
        {
            let value = self.eval_values_in(form, environment)?;
            return self.quoted_value_form(&value, form.span);
        }

        let expanded = self.expand_macros(form.clone(), environment)?;
        match &expanded.kind {
            FormKind::List(items) => self.prepare_compiled_list(&expanded, items, environment),
            _ => Ok(expanded),
        }
    }

    fn prepare_compiled_macrolet(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
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

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            let compiled = self.prepare_compiled_form(body_form, &local)?;
            self.note_compile_time_effect(&compiled, &local)?;
            prepared.push(compiled);
        }
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_compiled_symbol_macrolet(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
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

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            let compiled = self.prepare_compiled_form(body_form, &local)?;
            self.note_compile_time_effect(&compiled, &local)?;
            prepared.push(compiled);
        }
        Ok(Form::list(prepared, form.span))
    }

    fn prepare_compiled_list(
        &self,
        form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            if items.is_empty() {
                return Ok(form.clone());
            }
            let mut prepared = items.to_vec();
            prepared[0] = self.prepare_compiled_form(&items[0], environment)?;
            self.prepare_tail(&mut prepared, 1, environment)?;
            return Ok(Form::list(prepared, form.span));
        };

        let mut prepared = items.to_vec();
        match normalize_name(operator).as_str() {
            "QUOTE" | "QUASIQUOTE" => return Ok(form.clone()),
            "DECLARE"
            | "DECLAIM"
            | "PROCLAIM"
            | "DEFSTRUCT"
            | "DEFINE-CONDITION"
            | "DEFCLASS"
            | "DEFGENERIC"
            | "DEFMETHOD"
            | "DEFSETF"
            | "DEFINE-MODIFY-MACRO"
            | "DEFCONSTANT" => return Ok(form.clone()),
            "THE" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "LOCALLY" => {
                self.prepare_sequential_tail(&mut prepared, 1, environment)?;
            }
            "EVAL-WHEN" => {
                if prepared.len() > 1 && self.eval_when_executes(&prepared[1])? {
                    self.prepare_sequential_tail(&mut prepared, 2, environment)?;
                }
            }
            "PROGN"
            | "PROG1"
            | "PROG2"
            | "IF"
            | "WHEN"
            | "UNLESS"
            | "AND"
            | "OR"
            | "FUNCALL"
            | "APPLY"
            | "VALUES"
            | "IGNORE-ERRORS"
            | "UNWIND-PROTECT"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-LIST"
            | "MULTIPLE-VALUE-PROG1" => {
                self.prepare_sequential_tail(&mut prepared, 1, environment)?;
            }
            "WITH-SIMPLE-RESTART" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RESTART-CASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_restart_case_clause(clause, environment)?;
                }
            }
            "CATCH" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "PROGV" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "PROG" | "PROG*" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_prog_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DESTRUCTURING-BIND" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                if prepared.len() > 1 {
                    let local =
                        self.prepare_compiled_destructuring_environment(&prepared[1], environment)?;
                    self.prepare_tail(&mut prepared, 3, &local)?;
                } else {
                    self.prepare_tail(&mut prepared, 3, environment)?;
                }
            }
            "THROW" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "BLOCK" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RETURN" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
            }
            "RETURN-FROM" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
            }
            "MULTIPLE-VALUE-BIND" => {
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_compiled_form(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "MULTIPLE-VALUE-SETQ" => {
                return self.prepare_compiled_multiple_value_setq(form, &prepared, environment);
            }
            "LAMBDA" => {
                if prepared.len() > 1 {
                    let parameter_form = prepared[1].clone();
                    let local =
                        self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                    prepared[1] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                    self.prepare_tail(&mut prepared, 2, &local)?;
                } else {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "DEFUN" => {
                if prepared.len() > 2 {
                    let parameter_form = prepared[2].clone();
                    let local =
                        self.prepare_compiled_lambda_environment(&parameter_form, environment)?;
                    prepared[2] = self.prepare_compiled_lambda_list(&parameter_form, &local)?;
                    self.prepare_tail(&mut prepared, 3, &local)?;
                } else {
                    self.prepare_tail(&mut prepared, 3, environment)?;
                }
            }
            "FUNCTION" => {
                if prepared.len() == 2 && is_operator_form(&prepared[1], "LAMBDA") {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
            }
            "COND" => {
                for clause in prepared.iter_mut().skip(1) {
                    *clause = self.prepare_cond_clause(clause, environment)?;
                }
            }
            "CASE" | "ECASE" | "TYPECASE" | "ETYPECASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&items[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_case_clause(clause, environment)?;
                }
            }
            "HANDLER-CASE" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_compiled_form(&prepared[1], environment)?;
                }
                for clause in prepared.iter_mut().skip(2) {
                    *clause = self.prepare_handler_case_clause(clause, environment)?;
                }
            }
            "HANDLER-BIND" => {
                if prepared.len() > 1 {
                    let FormKind::List(handlers) = &prepared[1].kind else {
                        return Ok(Form::list(prepared, form.span));
                    };
                    let mut prepared_handlers = Vec::with_capacity(handlers.len());
                    for handler in handlers {
                        let FormKind::List(parts) = &handler.kind else {
                            prepared_handlers.push(handler.clone());
                            continue;
                        };
                        let mut prepared_parts = parts.to_vec();
                        if prepared_parts.len() > 1 {
                            prepared_parts[1] =
                                self.prepare_compiled_form(&parts[1], environment)?;
                        }
                        prepared_handlers.push(Form::list(prepared_parts, handler.span));
                    }
                    prepared[1] = Form::list(prepared_handlers, prepared[1].span);
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "RESTART-BIND" => {
                if prepared.len() > 1 {
                    let FormKind::List(bindings) = &prepared[1].kind else {
                        return Ok(Form::list(prepared, form.span));
                    };
                    let mut prepared_bindings = Vec::with_capacity(bindings.len());
                    for binding in bindings {
                        let FormKind::List(parts) = &binding.kind else {
                            prepared_bindings.push(binding.clone());
                            continue;
                        };
                        let mut prepared_parts = parts.to_vec();
                        if prepared_parts.len() > 1 {
                            prepared_parts[1] =
                                self.prepare_compiled_form(&parts[1], environment)?;
                        }
                        prepared_bindings.push(Form::list(prepared_parts, binding.span));
                    }
                    prepared[1] = Form::list(prepared_bindings, prepared[1].span);
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "LET" | "LET*" => {
                if prepared.len() > 1 {
                    let current = Form::list(prepared.clone(), form.span);
                    return self.prepare_compiled_let(
                        &current,
                        &prepared,
                        environment,
                        normalize_name(operator) == "LET*",
                    );
                } else {
                    self.prepare_tail(&mut prepared, 2, environment)?;
                }
            }
            "FLET" | "LABELS" => {
                if prepared.len() > 1 {
                    prepared[1] =
                        self.prepare_local_function_bindings(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DOTIMES" | "DOLIST" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_iteration_binding(&prepared[1], environment)?;
                }
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            "DO" | "DO*" => {
                if prepared.len() > 1 {
                    prepared[1] = self.prepare_do_bindings(&prepared[1], environment)?;
                }
                if prepared.len() > 2 {
                    prepared[2] = self.prepare_do_termination(&prepared[2], environment)?;
                }
                self.prepare_tail(&mut prepared, 3, environment)?;
            }
            "SETF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PSETF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PUSH" | "POP" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PUSHNEW" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "REMF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "ROTATEF" | "SHIFTF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "INCF" | "DECF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "PSETQ" => {
                return self.prepare_compiled_psetq(form, &prepared, environment);
            }
            "SETQ" => {
                return self.prepare_compiled_setq(form, &prepared, environment);
            }
            "DEFINE" | "DEFVAR" | "DEFPARAMETER" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
            }
            _ => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
        }

        Ok(Form::list(prepared, form.span))
    }

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

    fn prepare_tail(
        &self,
        items: &mut [Form],
        start: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for item in items.iter_mut().skip(start) {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(())
    }

    fn prepare_sequential_tail(
        &self,
        items: &mut [Form],
        start: usize,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        for item in items.iter_mut().skip(start) {
            *item = self.prepare_compiled_form(item, environment)?;
            self.note_compile_time_effect(item, environment)?;
        }
        Ok(())
    }

    fn note_compile_time_effect(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if is_operator_form(form, "DEFCONSTANT") {
            let FormKind::List(items) = &form.kind else {
                return Ok(());
            };
            if items.len() < 2 {
                return Ok(());
            }
            let (name, escaped) =
                self.variable_name_info(&items[1], "defconstant name must be a symbol")?;
            if escaped {
                environment.define_constant_exact(name);
            } else {
                environment.define_constant(name);
            }
            return Ok(());
        }

        let FormKind::List(items) = &form.kind else {
            return Ok(());
        };
        if normalize_name(atom_name(&items[0]).unwrap_or_default()) == "SETF"
            && items.len() == 3
            && is_operator_form(&items[1], "MACRO-FUNCTION")
        {
            self.eval_values_in(form, environment)?;
        }

        Ok(())
    }

    fn prepare_iteration_binding(
        &self,
        binding: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &binding.kind else {
            return Ok(binding.clone());
        };

        let mut prepared = items.to_vec();
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_form(&items[1], environment)?;
        }
        if prepared.len() > 2 {
            prepared[2] = self.prepare_compiled_form(&items[2], environment)?;
        }
        Ok(Form::list(prepared, binding.span))
    }

    fn prepare_do_bindings(
        &self,
        bindings: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(binding_forms) = &bindings.kind else {
            return Ok(bindings.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };

            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                prepared_parts[1] = self.prepare_compiled_form(&parts[1], environment)?;
            }
            if prepared_parts.len() > 2 {
                prepared_parts[2] = self.prepare_compiled_form(&parts[2], environment)?;
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }

        Ok(Form::list(prepared_bindings, bindings.span))
    }

    fn prepare_prog_bindings(
        &self,
        bindings: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(binding_forms) = &bindings.kind else {
            return Ok(bindings.clone());
        };

        let mut prepared_bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                prepared_bindings.push(binding.clone());
                continue;
            };

            let mut prepared_parts = parts.to_vec();
            if prepared_parts.len() > 1 {
                prepared_parts[1] = self.prepare_compiled_form(&parts[1], environment)?;
            }
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
        }

        Ok(Form::list(prepared_bindings, bindings.span))
    }

    fn prepare_do_termination(
        &self,
        termination: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(parts) = &termination.kind else {
            return Ok(termination.clone());
        };

        let mut prepared = Vec::with_capacity(parts.len());
        for part in parts {
            prepared.push(self.prepare_compiled_form(part, environment)?);
        }
        Ok(Form::list(prepared, termination.span))
    }

    fn prepare_cond_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        for item in &mut prepared {
            *item = self.prepare_compiled_form(item, environment)?;
        }
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        self.prepare_tail(&mut prepared, 1, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_handler_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn prepare_restart_case_clause(
        &self,
        clause: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &clause.kind else {
            return Ok(clause.clone());
        };

        let mut prepared = items.to_vec();
        if prepared.len() > 1 {
            prepared[1] = self.prepare_compiled_lambda_list(&items[1], environment)?;
        }
        self.prepare_tail(&mut prepared, 2, environment)?;
        Ok(Form::list(prepared, clause.span))
    }

    fn quoted_value_form(&self, value: &Value, span: Span) -> Result<Form, RuntimeError> {
        if let Value::Values(values) = value {
            let mut forms = vec![Form::atom("VALUES", span)];
            for value in values.iter() {
                forms.push(self.quoted_value_form(value, span)?);
            }
            return Ok(Form::list(forms, span));
        }

        Ok(Form::list(
            vec![
                Form::atom("QUOTE", span),
                self.form_from_value(value, span)?,
            ],
            span,
        ))
    }


    };
}

macro_rules! evaluator_errors {
    () => {
    fn arity(&self, function: &str, expected: &str, actual: usize) -> RuntimeError {
        RuntimeError::Arity {
            function: function.to_string(),
            expected: expected.to_string(),
            actual,
        }
    }

    fn block_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("block name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    fn restart_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("restart name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    fn condition_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid("condition name must be a symbol", form.span));
        };
        if name.is_empty()
            || (name.starts_with(':') && name.len() == 1)
            || (!name.starts_with(':')
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(self.invalid("condition name must be a symbol", form.span));
        }
        Ok(normalize_name(name).trim_start_matches(':').to_string())
    }

    fn variable_name_info(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(self.invalid(context, form.span));
        };
        if token.kind != SymbolTokenKind::Symbol
            || token.name.is_empty()
            || (token.escaped && token.package.is_some())
            || (!token.escaped && (token.name.starts_with('&') || literal_atom(name).is_some()))
        {
            return Err(self.invalid(context, form.span));
        }
        let variable_name = if token.escaped {
            token.name
        } else {
            normalize_name(name)
        };
        Ok((variable_name, token.escaped))
    }

    fn variable_name(&self, form: &Form, context: &str) -> Result<String, RuntimeError> {
        self.variable_name_info(form, context).map(|(name, _)| name)
    }

    fn define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) {
        if escaped {
            self.define_exact_in(name, value, environment);
        } else {
            self.define_in(name, value, environment);
        }
    }

    fn set_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) -> bool {
        if escaped {
            self.set_exact_in(name, value, environment)
        } else {
            self.set_in(name, value, environment)
        }
    }

    fn set_or_define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if escaped {
            self.set_or_define_exact_in(name, value, environment, span)
        } else {
            self.set_or_define_in(name, value, environment, span)
        }
    }

    fn ensure_symbol_writable(
        &self,
        name: &str,
        escaped: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let constant = if escaped {
            self.is_constant_exact_in(name)
        } else {
            self.is_constant_in(name)
        };
        if constant {
            Err(self.constant_modification_error(name, span))
        } else {
            Ok(())
        }
    }

    fn invalid(&self, message: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        }
    }

    };
}

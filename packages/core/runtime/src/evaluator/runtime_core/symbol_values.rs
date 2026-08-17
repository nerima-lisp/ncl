impl Runtime {
    pub(crate) fn define_dynamic(&self, name: &str, value: Value) {
        let binding_name = self
            .dynamic_candidates(name)
            .into_iter()
            .next()
            .unwrap_or_else(|| normalize_name(name));
        self.dynamic
            .borrow_mut()
            .bindings
            .push((binding_name, value));
    }

    pub(crate) fn define_special_value(&self, name: &str, value: Value, force: bool) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        if !force && let Some(existing) = dynamic.globals.get(&name) {
            return existing.clone();
        }
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_special_value_exact(
        &self,
        name: &str,
        value: Value,
        force: bool,
    ) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        if !force && let Some(existing) = dynamic.exact_globals.get(name) {
            return existing.clone();
        }
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn define_constant_value(&self, name: &str, value: Value) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        dynamic.constants.insert(name.clone());
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_constant_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        dynamic.exact_constants.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn lookup_special(&self, name: &str) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        candidates
            .iter()
            .find_map(|candidate| self.dynamic.borrow().globals.get(candidate).cloned())
    }

    pub(crate) fn lookup_special_exact(&self, name: &str) -> Option<Value> {
        self.dynamic.borrow().exact_globals.get(name).cloned()
    }

    pub(crate) fn is_constant_in(&self, name: &str) -> bool {
        self.dynamic_candidates(name)
            .into_iter()
            .any(|candidate| self.dynamic.borrow().constants.contains(&candidate))
    }

    pub(crate) fn is_constant_exact_in(&self, name: &str) -> bool {
        self.dynamic.borrow().exact_constants.contains(name)
    }

    pub(crate) fn constantp_in(&self, value: &Value, environment: Option<&Environment>) -> bool {
        match value {
            Value::Nil
            | Value::Boolean(_)
            | Value::Integer(_)
            | Value::Rational(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Character(_)
            | Value::Keyword(_)
            | Value::KeywordExact(_) => true,
            Value::Symbol(name) => match environment.and_then(|env| env.constant_status(name)) {
                Some(status) => status,
                None => {
                    name.eq_ignore_ascii_case("T")
                        || name.eq_ignore_ascii_case("NIL")
                        || self.is_constant_in(name)
                }
            },
            Value::SymbolExact(name) => {
                match environment.and_then(|env| env.constant_status_exact(name)) {
                    Some(status) => status,
                    None => {
                        name.eq_ignore_ascii_case("T")
                            || name.eq_ignore_ascii_case("NIL")
                            || self.is_constant_exact_in(name)
                    }
                }
            }
            Value::List(items) => matches!(
                &items.as_ref()[..],
                [Value::Symbol(name) | Value::SymbolExact(name), _]
                    if name.eq_ignore_ascii_case("QUOTE")
            ),
            _ => false,
        }
    }

    pub(crate) fn constant_modification_error(&self, name: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: format!("cannot modify constant {name}"),
            span: Some(span),
        }
    }

    pub(crate) fn set_or_define_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_in(name) {
            return Err(self.constant_modification_error(name, span));
        }
        self.define_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_or_define_exact_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_exact_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_exact_in(name) {
            return Err(self.constant_modification_error(name, span));
        }
        self.define_exact_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_symbol_value(&self, name: &str, value: Value) -> Value {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic
            .bindings
            .iter_mut()
            .rev()
            .find(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
        {
            *current = value.clone();
            return value;
        }
        let global_name = candidates
            .iter()
            .find(|candidate| dynamic.special_names.contains(*candidate))
            .cloned()
            .unwrap_or_else(|| normalize_name(name));
        dynamic.special_names.insert(global_name.clone());
        dynamic.globals.insert(global_name, value.clone());
        value
    }

    pub(crate) fn set_symbol_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic
            .exact_bindings
            .iter_mut()
            .rev()
            .find(|(binding, _)| binding == name)
        {
            *current = value.clone();
            return value;
        }
        dynamic.exact_special_names.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn makunbound_symbol(&self, name: &str) {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        for candidate in candidates {
            dynamic.globals.remove(&candidate);
        }
    }
}

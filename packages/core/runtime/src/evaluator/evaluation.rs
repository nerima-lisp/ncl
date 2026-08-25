use super::*;

impl Runtime {
    pub fn new() -> Self {
        let global = Environment::new();
        builtins::install(&global);
        Self {
            global,
            packages: Rc::new(RefCell::new(PackageState::new())),
            dynamic: Rc::new(RefCell::new(DynamicState::default())),
            next_block_target: Cell::new(1),
            gensym_counter: Cell::new(0),
            method_context: RefCell::new(Vec::new()),
        }
    }

    pub fn global_environment(&self) -> Environment {
        self.global.clone()
    }

    pub fn current_package(&self) -> String {
        self.packages.borrow().current().to_string()
    }

    pub(crate) fn fresh_block_target(&self) -> u64 {
        let target = self.next_block_target.get();
        self.next_block_target.set(target.wrapping_add(1));
        target
    }

    pub(super) fn package_error(&self, message: &str, span: Span) -> RuntimeError {
        RuntimeError::Package {
            message: message.to_string(),
            span: Some(span),
        }
    }

    pub(crate) fn dynamic_guard(&self) -> DynamicGuard {
        DynamicGuard {
            state: self.dynamic.clone(),
            depth: self.dynamic.borrow().bindings.len(),
            exact_depth: self.dynamic.borrow().exact_bindings.len(),
        }
    }

    pub(crate) fn condition_handler_guard(
        &self,
        handlers: Vec<ConditionHandlerBinding>,
    ) -> ConditionHandlerGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.condition_handlers.len();
        state.condition_handlers.extend(handlers);
        ConditionHandlerGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn condition_handlers(&self) -> Vec<ConditionHandlerBinding> {
        self.dynamic.borrow().condition_handlers.clone()
    }

    pub(crate) fn suspend_condition_handler(
        &self,
        condition: &str,
    ) -> Option<ConditionHandlerSuspension> {
        let condition = normalize_name(condition);
        let mut state = self.dynamic.borrow_mut();
        let index = state
            .condition_handlers
            .iter()
            .rposition(|handler| normalize_name(&handler.condition) == condition)?;
        let binding = state.condition_handlers.remove(index);
        Some(ConditionHandlerSuspension {
            state: self.dynamic.clone(),
            index,
            binding: Some(binding),
        })
    }

    pub(crate) fn restart_guard(&self, bindings: Vec<RestartBinding>) -> RestartGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.restart_bindings.len();
        state.restart_bindings.extend(bindings);
        RestartGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn restart_bindings(&self) -> Vec<RestartBinding> {
        self.dynamic.borrow().restart_bindings.clone()
    }

    pub(crate) fn condition_restart_guard(
        &self,
        condition: Value,
        restarts: Vec<Value>,
    ) -> ConditionRestartGuard {
        let mut state = self.dynamic.borrow_mut();
        let depth = state.condition_restart_bindings.len();
        state
            .condition_restart_bindings
            .push(ConditionRestartBinding {
                condition,
                restarts,
            });
        ConditionRestartGuard {
            state: self.dynamic.clone(),
            depth,
        }
    }

    pub(crate) fn condition_restart_bindings(&self) -> Vec<ConditionRestartBinding> {
        self.dynamic.borrow().condition_restart_bindings.clone()
    }

    pub(crate) fn restart_bindings_for_condition(
        &self,
        condition: Option<&Value>,
    ) -> Vec<RestartBinding> {
        let bindings = self.restart_bindings();
        let Some(condition) = condition else {
            return bindings;
        };
        let associations = self.condition_restart_bindings();
        bindings
            .into_iter()
            .filter(|binding| {
                let associated_with_condition = associations.iter().any(|association| {
                    association.condition.eq_value(condition)
                        && association
                            .restarts
                            .iter()
                            .any(|restart| restart.eq_value(&binding.restart))
                });
                let associated_with_any_condition = associations.iter().any(|association| {
                    association
                        .restarts
                        .iter()
                        .any(|restart| restart.eq_value(&binding.restart))
                });
                associated_with_condition || !associated_with_any_condition
            })
            .collect()
    }

    pub(crate) fn dynamic_depth(&self) -> usize {
        self.dynamic.borrow().bindings.len()
    }

    pub(crate) fn truncate_dynamic(&self, depth: usize) {
        self.dynamic.borrow_mut().bindings.truncate(depth);
    }

    pub(crate) fn exact_dynamic_depth(&self) -> usize {
        self.dynamic.borrow().exact_bindings.len()
    }

    pub(crate) fn truncate_exact_dynamic(&self, depth: usize) {
        self.dynamic.borrow_mut().exact_bindings.truncate(depth);
    }

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

    pub(crate) fn constantp(&self, value: &Value) -> bool {
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
            Value::Symbol(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_in(name)
            }
            Value::SymbolExact(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_exact_in(name)
            }
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

    pub(super) fn remove_global_symbol(&self, name: &str) {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.globals.remove(name);
        dynamic.special_names.remove(name);
        dynamic.constants.remove(name);
        drop(dynamic);
        self.global.remove(name);
        self.global.remove_function(name);
    }

    pub(crate) fn makunbound_exact_symbol(&self, name: &str) {
        self.dynamic.borrow_mut().exact_globals.remove(name);
    }

    pub(crate) fn fmakunbound_symbol(&self, name: &str) {
        for candidate in self.dynamic_candidates(name) {
            self.global.remove(&candidate);
            self.global.remove_function(&candidate);
        }
    }

    pub(crate) fn fmakunbound_exact_symbol(&self, name: &str) {
        self.global.remove_exact(name);
        self.global.remove_function_exact(name);
    }

    pub(super) fn dynamic_candidates(&self, name: &str) -> Vec<String> {
        let qualified = package::split_symbol(name).is_some();
        let (package_name, symbol_name) = match package::split_symbol(name) {
            Some((package_name, symbol_name, _)) => (
                package::normalize_package_name(package_name),
                normalize_name(symbol_name),
            ),
            None => (self.current_package(), normalize_name(name)),
        };
        let packages = self.packages.borrow();
        let package_name = packages.canonical_package_name(&package_name);
        let mut candidates = Vec::new();
        if let Some(imported) = packages.imported_symbol_for(&package_name, &symbol_name) {
            candidates.push(imported);
        } else if qualified {
            candidates.push(package::canonical_symbol_name(&package_name, &symbol_name));
        } else {
            candidates.push(normalize_name(name));
        }
        if !packages.is_shadowed(&package_name, &symbol_name) {
            for used in packages.use_packages_for(&package_name) {
                if packages.is_exported(&used, &symbol_name) {
                    let candidate = format!("{used}::{symbol_name}");
                    if !candidates.contains(&candidate) {
                        candidates.push(candidate);
                    }
                }
            }
        }
        candidates
    }

    pub(super) fn symbol_macro_expansion_for_atom(
        &self,
        atom: &str,
        environment: &Environment,
    ) -> Option<Form> {
        if literal_atom(atom).is_some() {
            return None;
        }

        let (name, escaped) = resolved_symbol(atom);
        if escaped {
            environment.lookup_symbol_macro_exact(&name)
        } else {
            environment.lookup_symbol_macro(&name)
        }
    }

    pub(super) fn expand_symbol_macro_form(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let mut current = form.clone();
        let mut expanded = false;
        let mut seen = HashSet::new();

        loop {
            let Some(atom) = atom_name(&current) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let Some(next) = self.symbol_macro_expansion_for_atom(atom, environment) else {
                return Ok(if expanded { Some(current) } else { None });
            };
            let (name, escaped) = resolved_symbol(atom);
            let key = format!("{}:{}", if escaped { "escaped" } else { "normal" }, name);
            if !seen.insert(key) {
                return Err(self.invalid("recursive symbol macro expansion", form.span));
            }
            expanded = true;
            current = next;
        }
    }

    pub(super) fn prepare_compiled_form(
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

        if is_operator_form(form, "DEFMACRO")
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

    pub(super) fn prepare_compiled_macrolet(
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
            let lambda_list = self.macro_parameters(&parts[1])?;
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
            prepared.push(self.prepare_compiled_form(body_form, &local)?);
        }
        Ok(Form::list(prepared, form.span))
    }

    pub(super) fn prepare_compiled_symbol_macrolet(
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
            prepared.push(self.prepare_compiled_form(body_form, &local)?);
        }
        Ok(Form::list(prepared, form.span))
    }

    pub(super) fn prepare_compiled_list(
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
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "EVAL-WHEN" => {
                if prepared.len() > 1 && self.eval_when_executes(&prepared[1])? {
                    self.prepare_tail(&mut prepared, 2, environment)?;
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
                self.prepare_tail(&mut prepared, 1, environment)?;
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
                self.prepare_tail(&mut prepared, 3, environment)?;
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
            "ROTATEF" | "SHIFTF" => {
                self.prepare_tail(&mut prepared, 1, environment)?;
            }
            "INCF" | "DECF" => {
                self.prepare_tail(&mut prepared, 2, environment)?;
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

    pub(super) fn prepare_compiled_lambda_environment(
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

    pub(super) fn prepare_compiled_let(
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
            prepared_bindings.push(Form::list(prepared_parts, binding.span));
            if escaped {
                local.define_exact(name, Value::Nil);
            } else {
                local.define(name, Value::Nil);
            }
        }

        let mut prepared = items.to_vec();
        prepared[1] = Form::list(prepared_bindings, binding_form.span);
        self.prepare_tail(&mut prepared, 2, &local)?;
        Ok(Form::list(prepared, form.span))
    }

    pub(super) fn prepare_compiled_setq(
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

    pub(super) fn prepare_compiled_psetq(
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

    pub(super) fn prepare_compiled_multiple_value_setq(
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

    pub(super) fn symbol_macro_temporary(&self, form: &Form, index: usize, span: Span) -> Form {
        Form::atom(
            format!(
                "NCL-SYMBOL-MACRO-TEMP-{}-{}-{}",
                form.span.start, form.span.end, index
            ),
            span,
        )
    }

    pub(super) fn prepare_compiled_lambda_list(
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

    pub(super) fn prepare_local_function_bindings(
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

    pub(super) fn prepare_tail(
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

    pub(super) fn prepare_iteration_binding(
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

    pub(super) fn prepare_do_bindings(
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

    pub(super) fn prepare_prog_bindings(
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

    pub(super) fn prepare_do_termination(
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

    pub(super) fn prepare_cond_clause(
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

    pub(super) fn prepare_case_clause(
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

    pub(super) fn prepare_handler_case_clause(
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

    pub(super) fn prepare_restart_case_clause(
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

    pub(super) fn quoted_value_form(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Form, RuntimeError> {
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

    pub(crate) fn eval_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_values_in(form, environment)
            .map(|value| value.primary_value())
    }

    pub(crate) fn eval_values_in(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(expanded) = self.expand_symbol_macro_form(form, environment)? {
                    return self.eval_values_in(&expanded, environment);
                }
                self.eval_atom(atom, form.span, environment)
            }
            FormKind::String(value) => Ok(Value::string(value.clone())),
            FormKind::Character(value) => Ok(Value::Character(*value)),
            FormKind::Vector(items) => Ok(Value::vector(
                items
                    .iter()
                    .map(|item| self.quoted_value(item))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { .. } => {
                Err(self.invalid("cannot evaluate a dotted list", form.span))
            }
            FormKind::List(items) => self.eval_list_values(items, form.span, environment),
        }
    }
}

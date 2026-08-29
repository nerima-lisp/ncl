#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_destructuring_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "destructuring-bind",
                "two or more",
                items.len().saturating_sub(1),
            ));
        }
        let lambda_list = match &items[1].kind {
            FormKind::List(_) => {
                let lambda_list = Self::macro_parameters(&items[1])?;
                if lambda_list.environment.is_some() {
                    return Err(Self::invalid(
                        "&environment is only valid in macro lambda lists",
                        items[1].span,
                    ));
                }
                Some(lambda_list)
            }
            _ => None,
        };
        let mut seen = HashSet::new();
        let pattern = lambda_list
            .is_none()
            .then(|| Self::macro_pattern(&items[1], &mut seen));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = &lambda_list
            && let Some(whole) = &lambda_list.whole
        {
            local.define(whole, value.clone());
        }
        if let Some(lambda_list) = lambda_list {
            self.bind_destructuring_lambda_list(&lambda_list, &value, &local, items[1].span)?;
        } else if let Some(pattern) = pattern {
            Self::bind_macro_pattern(&pattern, value, &local, items[1].span)?;
        }
        self.eval_sequence_values(&items[3..], &local)
    }

    pub(crate) fn special_let(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                if sequential { "let*" } else { "let" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid("let bindings must be a list", items[1].span));
        };
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(Self::invalid("let binding must be a list", binding.span));
            };
            if !(binding_items.len() == 1 || binding_items.len() == 2) {
                return Err(Self::invalid(
                    "let binding needs a name and optional value",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&binding_items[0], "let binding name must be a symbol")?;
            let value = binding_items.get(1).map_or(Ok(Value::Nil), |form| {
                self.eval_in(form, if sequential { &local } else { environment })
            })?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    pub(crate) fn special_flet(
        &self,
        items: &[Form],
        environment: &Environment,
        recursive: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if recursive { "labels" } else { "flet" };
        if items.len() < 2 {
            return Err(Self::arity(
                operator,
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "local function bindings must be a list",
                items[1].span,
            ));
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
                return Err(Self::invalid(
                    "local function binding must be a list",
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(Self::invalid(
                    "local function needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (normalized, escaped) =
                Self::variable_name_info(&parts[0], "local function name must be a symbol")?;
            if !names.insert(normalized.clone()) {
                return Err(Self::invalid(
                    "local function names must be unique",
                    parts[0].span,
                ));
            }
            definitions.push((
                normalized,
                escaped,
                Self::parameters(&parts[1])?,
                parts[2..].to_vec(),
            ));
        }

        for (name, escaped, lambda_list, body) in definitions {
            let function = Value::closure_with_keywords(
                crate::ClosureOptions {
                    parameters: lambda_list.required,
                    required_escaped: lambda_list.required_escaped,
                    optional: lambda_list.optional,
                    rest: lambda_list.rest,
                    rest_escaped: lambda_list.rest_escaped,
                    keywords: lambda_list.keywords,
                    has_keyword_section: lambda_list.has_keyword_section,
                    allow_other_keys: lambda_list.allow_other_keys,
                    auxiliary: lambda_list.auxiliary,
                },
                body,
                captured.clone(),
            );
            if escaped {
                local.define_function_exact(name, function);
            } else {
                local.define_function(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    pub(crate) fn special_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "macrolet",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "local macro bindings must be a list",
                items[1].span,
            ));
        };

        let local = environment.child();
        let captured = environment.clone();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid(
                    "local macro binding must be a list",
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(Self::invalid(
                    "local macro needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&parts[0], "local macro name must be a symbol")?;
            if !names.insert(name.clone()) {
                return Err(Self::invalid(
                    "local macro names must be unique",
                    parts[0].span,
                ));
            }
            let lambda_list = Self::macro_parameters(&parts[1])?;
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

    pub(crate) fn special_symbol_macrolet(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "symbol-macrolet",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "symbol macro bindings must be a list",
                items[1].span,
            ));
        };

        let local = environment.child();
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid(
                    "symbol macro binding must be a list",
                    binding.span,
                ));
            };
            if parts.len() != 2 {
                return Err(Self::invalid(
                    "symbol macro binding needs a name and an expansion",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&parts[0], "symbol macro name must be a symbol")?;
            if !names.insert((name.clone(), escaped)) {
                return Err(Self::invalid(
                    "symbol macro names must be unique",
                    parts[0].span,
                ));
            }
            if escaped {
                local.define_symbol_macro_exact(name, parts[1].clone());
            } else {
                local.define_symbol_macro(name, parts[1].clone());
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }

    pub(crate) fn special_define_symbol_macro(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity(
                "DEFINE-SYMBOL-MACRO",
                "two",
                items.len().saturating_sub(1),
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&items[1], "DEFINE-SYMBOL-MACRO name must be a symbol")?;
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

    pub(crate) fn special_dotimes(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "dotimes",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "dotimes binding must be a list",
                items[1].span,
            ));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(Self::invalid(
                "dotimes binding needs a name, count, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&binding[0], "dotimes binding name must be a symbol")?;
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

    pub(crate) fn special_dolist(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "dolist",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "dolist binding must be a list",
                items[1].span,
            ));
        };
        if !(binding.len() == 2 || binding.len() == 3) {
            return Err(Self::invalid(
                "dolist binding needs a name, list, and optional result",
                items[1].span,
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&binding[0], "dolist binding name must be a symbol")?;
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

    pub(crate) fn special_do(
        &self,
        items: &[Form],
        environment: &Environment,
        sequential: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if sequential { "do*" } else { "do" };
        if items.len() < 3 {
            return Err(Self::arity(
                operator,
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding_forms) = &items[1].kind else {
            return Err(Self::invalid("do bindings must be a list", items[1].span));
        };
        let FormKind::List(termination) = &items[2].kind else {
            return Err(Self::invalid(
                "do termination must be a list",
                items[2].span,
            ));
        };
        if termination.is_empty() {
            return Err(Self::invalid(
                "do termination needs an end test",
                items[2].span,
            ));
        }

        let bindings = Self::parse_do_bindings(binding_forms)?;

        let target = self.fresh_block_target();
        let block_environment = environment.child();
        block_environment.define_block("NIL", target);
        let local = block_environment.child();
        let _dynamic_guard = self.dynamic_guard();

        let initialization =
            self.initialize_do_bindings(&bindings, sequential, &local, &block_environment);
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
                self.advance_do_bindings(&bindings, sequential, &local)?;
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

    pub(super) fn initialize_do_bindings(
        &self,
        bindings: &[DoBinding],
        sequential: bool,
        local: &Environment,
        block_environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if sequential {
            for (name, escaped, init, _) in bindings {
                let value = init
                    .as_ref()
                    .map_or(Ok(Value::Nil), |form| self.eval_in(form, local))?;
                self.define_variable_in(name, *escaped, value, local);
            }
        } else {
            let mut values = Vec::with_capacity(bindings.len());
            for (_, _, init, _) in bindings {
                values.push(
                    init.as_ref()
                        .map_or(Ok(Value::Nil), |form| self.eval_in(form, block_environment))?,
                );
            }
            for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                self.define_variable_in(name, *escaped, value, local);
            }
        }
        Ok(())
    }

    pub(super) fn advance_do_bindings(
        &self,
        bindings: &[DoBinding],
        sequential: bool,
        local: &Environment,
    ) -> Result<(), RuntimeError> {
        if sequential {
            for (name, escaped, _, step) in bindings {
                if let Some(step) = step {
                    let value = self.eval_in(step, local)?;
                    self.set_variable_in(name, *escaped, value, local);
                }
            }
        } else {
            let mut values = Vec::with_capacity(bindings.len());
            for (_, _, _, step) in bindings {
                values.push(match step {
                    Some(step) => Some(self.eval_in(step, local)?),
                    None => None,
                });
            }
            for ((name, escaped, _, _), value) in bindings.iter().zip(values) {
                if let Some(value) = value {
                    self.set_variable_in(name, *escaped, value, local);
                }
            }
        }
        Ok(())
    }

    pub(super) fn parse_do_bindings(
        binding_forms: &[Form],
    ) -> Result<Vec<DoBinding>, RuntimeError> {
        let mut names = HashSet::new();
        let mut bindings = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid("do binding must be a list", binding.span));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(Self::invalid(
                    "do binding needs a name, optional init, and optional step",
                    binding.span,
                ));
            }
            let (name, escaped) =
                Self::variable_name_info(&parts[0], "do binding name must be a symbol")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(Self::invalid(
                    "do binding names must be unique",
                    parts[0].span,
                ));
            }
            bindings.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }
        Ok(bindings)
    }
}

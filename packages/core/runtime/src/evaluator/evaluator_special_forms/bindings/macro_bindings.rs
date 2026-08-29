use super::{Environment, Form, FormKind, HashSet, Runtime, RuntimeError, Value};

impl Runtime {
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
}

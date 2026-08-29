#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn prepare_compiled_macrolet(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
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

        let mut prepared = Vec::with_capacity(items.len().saturating_sub(2) + 1);
        prepared.push(Form::atom("PROGN", form.span));
        for body_form in &items[2..] {
            prepared.push(self.prepare_compiled_form(body_form, &local)?);
        }
        Ok(Form::list(prepared, form.span))
    }
}

use super::{
    Environment, Form, MacroPattern, Runtime, RuntimeError, Value, atom_name, resolved_symbol,
};

impl Runtime {
    pub(crate) fn special_defmacro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "defmacro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "defmacro name must be a symbol",
                items[1].span,
            ));
        };
        let lambda_list = Self::macro_parameters(&items[2])?;
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

    pub(crate) fn special_define_modify_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "define-modify-macro needs a name, parameters, and a function",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "define-modify-macro name must be a symbol",
                items[1].span,
            ));
        };
        let mut lambda_list = Self::macro_parameters(&items[2])?;
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

    pub(crate) fn special_macroexpand_1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "macroexpand-1",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = Self::form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self
            .expand_macro_once(&form, &expansion_environment)?
            .map_or((form, false), |expanded| (expanded, true));
        Ok(Value::values(vec![
            Self::quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }
}

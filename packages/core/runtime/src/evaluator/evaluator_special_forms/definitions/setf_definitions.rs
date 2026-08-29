use super::{
    Environment, Form, Runtime, RuntimeError, Value, atom_name, resolved_symbol, unqualified_name,
};

impl Runtime {
    pub(crate) fn special_defsetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::invalid(
                "DEFSETF needs an accessor and a writer",
                items[0].span,
            ));
        }
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "DEFSETF accessor must be a symbol",
                items[1].span,
            ));
        };

        let writer_designator = if let Some(writer) = atom_name(&items[2]) {
            let (resolved_name, escaped) = resolved_symbol(writer);
            if escaped {
                Value::symbol_exact(resolved_name)
            } else {
                Value::symbol(resolved_name)
            }
        } else {
            self.eval_in(&items[2], environment)?
        };
        let writer = Value::Function(self.resolve_function_designator(
            &writer_designator,
            items[2].span,
            environment,
        )?);
        let (resolved_name, escaped) = resolved_symbol(accessor);
        environment.define_setf_function(unqualified_name(&resolved_name), writer);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    pub(crate) fn special_define_setf_expander(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "DEFINE-SETF-EXPANDER needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "DEFINE-SETF-EXPANDER name must be a symbol",
                items[1].span,
            ));
        };
        let lambda_list = Self::macro_parameters(&items[2])?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        environment.define_setf_expander(unqualified_name(&resolved_name), function);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    pub(crate) fn special_get_setf_expansion(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "GET-SETF-EXPANSION",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place_value = self.eval_in(&items[1], environment)?;
        let place = Self::form_from_value(&place_value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let expansion = self.get_setf_expansion(&place, &expansion_environment)?;
        Self::setf_expansion_value(&expansion, items[0].span)
    }
}

use super::{Environment, Form, Runtime, RuntimeError, Value, atom_name, normalize_name};

impl Runtime {
    pub(crate) fn special_modify_symbol(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity(
                operator,
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place = &items[1];
        if atom_name(place).is_some()
            && Self::expand_symbol_macro_form(place, environment)?.is_none()
        {
            Self::variable_name(place, &format!("{operator} target"))?;
        }
        let current = self.eval_in(place, environment)?;
        let delta = items
            .get(2)
            .map(|form| self.eval_in(form, environment))
            .transpose()?
            .unwrap_or(Value::Integer(1));
        let function = self
            .lookup_function_in(arithmetic, environment)
            .ok_or_else(|| RuntimeError::UnboundVariable {
                name: normalize_name(arithmetic),
                span: Some(items[0].span),
            })?;
        let value = self
            .apply_in(&function, &[current, delta], items[0].span, environment)?
            .primary_value();
        self.set_place(place, value.clone(), environment)?;
        Ok(value)
    }
}

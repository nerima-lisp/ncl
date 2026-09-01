use super::{Environment, Form, Runtime, RuntimeError, Value, atom_name, normalize_name};

impl Runtime {
    pub(crate) fn special_remf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("REMF", "two", items.len().saturating_sub(1)));
        }
        let expansion = self.get_modify_macro_setf_expansion(&items[1], environment)?;
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        let current = self.eval_in(&expansion.access_form, &local)?;
        let indicator = self.eval_in(&items[2], &local)?;
        let mut properties = current.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(items[1].span),
        })?;
        if !properties.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "REMF needs an even property list",
                items[1].span,
            ));
        }
        let found_index = (0..properties.len())
            .step_by(2)
            .find(|&index| crate::builtins::eql_value(&properties[index], &indicator));
        let found = found_index.is_some();
        if let Some(index) = found_index {
            properties.drain(index..=index + 1);
        }
        let new_plist = Value::list(properties);
        let (store_name, store_escaped) =
            Self::variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, new_plist.clone(), &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(Value::values(vec![new_plist, Value::boolean(found)]))
    }

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

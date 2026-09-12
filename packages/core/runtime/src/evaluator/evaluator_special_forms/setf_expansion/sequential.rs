use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError, Value, atom_name};

impl Runtime {
    pub(crate) fn execute_sequential_assignment(
        &self,
        items: &[Form],
        environment: &Environment,
        mut evaluate_value: impl FnMut(usize) -> Result<Value, RuntimeError>,
    ) -> Result<Value, RuntimeError> {
        let mut context = SetfTemporaryContext::default();
        for item in items {
            context.reserve_form(item, environment);
        }
        let mut result = Value::Nil;
        for (index, pair) in items[1..].as_chunks::<2>().0.iter().enumerate() {
            let _guard = self.dynamic_guard();
            let place = Self::expand_symbol_macro_form(&pair[0], environment)?
                .unwrap_or_else(|| pair[0].clone());
            if atom_name(&place).is_some() {
                Self::variable_name_info(&place, "SETF target must be a symbol")?;
                result = evaluate_value(index)?.primary_value();
                self.set_place(&place, result.clone(), environment)?;
                continue;
            }
            let expansion = self.parallel_setf_expansion(&place, environment, &mut context)?;
            if expansion.temporaries.len() != expansion.values.len() {
                return Err(Self::invalid(
                    "SETF expansion temporary and value lists must have the same length",
                    pair[0].span,
                ));
            }
            let temporaries = expansion
                .temporaries
                .iter()
                .map(|temporary| {
                    Self::variable_name_info(temporary, "SETF temporary must be a symbol")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let local = environment.child();
            for ((name, escaped), form) in temporaries.iter().zip(&expansion.values) {
                let value = self.eval_in(form, &local)?.primary_value();
                self.define_variable_in(name, *escaped, value, &local);
            }
            // Expander lexical temporaries must not become visible to the caller's RHS.
            let value = evaluate_value(index)?;
            self.bind_setf_stores(&expansion.stores, &value, &local)?;
            result = self.eval_values_in(&expansion.store_form, &local)?;
        }
        Ok(result)
    }
}

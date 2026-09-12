use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError, SetfExpansion, Value};

impl Runtime {
    fn capture_transfer_place(
        &self,
        place: &Form,
        environment: &Environment,
        context: &mut SetfTemporaryContext,
    ) -> Result<(SetfExpansion, Environment), RuntimeError> {
        let expansion = self.parallel_setf_expansion(place, environment, context)?;
        context.reserve_expansion(&expansion, environment);
        let local = environment.child();
        for (temporary, form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        Ok((expansion, local))
    }

    pub(crate) fn capture_modify_place(
        &self,
        place: &Form,
        invocation: &[Form],
        environment: &Environment,
    ) -> Result<(SetfExpansion, Environment), RuntimeError> {
        let mut context = SetfTemporaryContext::default();
        for form in invocation {
            context.reserve_form(form, environment);
        }
        self.capture_transfer_place(place, environment, &mut context)
    }

    pub(crate) fn store_modify_place(
        &self,
        expansion: &SetfExpansion,
        value: &Value,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.bind_setf_stores(&expansion.stores, value, environment)?;
        self.eval_in(&expansion.store_form, environment)?;
        self.eval_values_in(
            &Self::setf_store_values_form(&expansion.stores, expansion.store_form.span),
            environment,
        )
    }

    pub(crate) fn execute_place_transfer(
        &self,
        items: &[Form],
        environment: &Environment,
        shift: bool,
    ) -> Result<Value, RuntimeError> {
        let mut context = SetfTemporaryContext::default();
        for item in items {
            context.reserve_form(item, environment);
        }
        let _guard = self.dynamic_guard();
        let end = items.len() - usize::from(shift);
        let mut captures = Vec::new();
        for place in &items[1..end] {
            captures.push(self.capture_transfer_place(place, environment, &mut context)?);
        }
        let values = captures
            .iter()
            .map(|(expansion, local)| self.eval_values_in(&expansion.access_form, local))
            .collect::<Result<Vec<_>, _>>()?;
        let first = values.first().cloned().unwrap_or(Value::Nil);
        let last = if shift {
            self.eval_values_in(&items[end], environment)?
        } else {
            first.clone()
        };
        let incoming = if shift {
            values
                .into_iter()
                .skip(1)
                .chain(std::iter::once(last))
                .collect::<Vec<_>>()
        } else {
            values
                .into_iter()
                .skip(1)
                .chain(std::iter::once(first.clone()))
                .collect::<Vec<_>>()
        };
        for ((expansion, local), value) in captures.into_iter().zip(incoming) {
            self.bind_setf_stores(&expansion.stores, &value, &local)?;
            self.eval_in(&expansion.store_form, &local)?;
        }
        Ok(if shift { first } else { Value::Nil })
    }
}

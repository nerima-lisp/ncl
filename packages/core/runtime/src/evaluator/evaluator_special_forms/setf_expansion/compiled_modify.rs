use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, FormKind, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn execute_compiled_modify_place(
        &self,
        invocation: &Form,
        arithmetic: &str,
        environment: &Environment,
        delta: impl FnOnce() -> Result<Value, RuntimeError>,
    ) -> Result<Value, RuntimeError> {
        let FormKind::List(items) = &invocation.kind else {
            return Err(Self::invalid(
                "invalid modifying invocation",
                invocation.span,
            ));
        };
        let place = items
            .get(1)
            .ok_or_else(|| Self::invalid("missing modifying place", invocation.span))?;
        let mut context = SetfTemporaryContext::default();
        context.reserve_form(invocation, environment);
        let expansion =
            self.get_modify_macro_setf_expansion_with_context(place, environment, &mut context)?;
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(Self::invalid(
                "SETF expansion temporary and value lists must have the same length",
                invocation.span,
            ));
        }
        let temporaries = expansion
            .temporaries
            .iter()
            .map(|temporary| Self::variable_name_info(temporary, "SETF temporary must be a symbol"))
            .collect::<Result<Vec<_>, _>>()?;
        let _guard = self.dynamic_guard();
        let local = environment.child();
        for ((name, escaped), form) in temporaries.iter().zip(&expansion.values) {
            let value = self.eval_in(form, &local)?.primary_value();
            self.define_variable_in(name, *escaped, value, &local);
        }
        // The delta must run in the caller environment, outside expander lexical bindings,
        // after place subforms but before the old-value read.
        let delta = delta()?.primary_value();
        let old = self
            .eval_in(&expansion.access_form, &local)?
            .primary_value();
        let value = self
            .apply_in(
                &Value::Symbol(arithmetic.into()),
                &[old, delta],
                invocation.span,
                environment,
            )?
            .primary_value();
        self.bind_setf_stores(&expansion.stores, &value, &local)?;
        self.eval_in(&expansion.store_form, &local)?;
        let result = self.eval_values_in(
            &Self::setf_store_values_form(&expansion.stores, invocation.span),
            &local,
        )?;
        Ok(if expansion.stores.len() == 1 {
            result.primary_value()
        } else {
            result
        })
    }
}

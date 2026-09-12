use super::temporary::SetfTemporaryContext;
use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn execute_parallel_assignment(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut context = SetfTemporaryContext::default();
        for item in items {
            context.reserve_form(item, environment);
        }
        let _guard = self.dynamic_guard();
        let mut assignments = Vec::with_capacity(items.len() / 2);
        for pair in items[1..].as_chunks::<2>().0 {
            let expansion = self.parallel_setf_expansion(&pair[0], environment, &mut context)?;
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
            let value = self.eval_values_in(&pair[1], environment)?;
            self.bind_setf_stores(&expansion.stores, &value, &local)?;
            assignments.push((expansion.store_form, local));
        }
        for (store_form, local) in assignments {
            self.eval_in(&store_form, &local)?;
        }
        Ok(Value::Nil)
    }
}

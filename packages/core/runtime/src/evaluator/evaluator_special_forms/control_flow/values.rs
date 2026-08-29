use ncl_syntax::Form;

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_values(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let values = items[1..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::values(values))
    }

    pub(crate) fn special_multiple_value_list(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "multiple-value-list",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let values = self
            .eval_values_in(&items[1], environment)?
            .multiple_values();
        Ok(Value::list(values))
    }
}

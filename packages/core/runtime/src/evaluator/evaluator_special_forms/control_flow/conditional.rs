use ncl_syntax::Form;

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_if(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity(
                "if",
                "two or three",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?;
        if condition.is_truthy() {
            self.eval_values_in(&items[2], environment)
        } else {
            items.get(3).map_or(Ok(Value::Nil), |form| {
                self.eval_values_in(form, environment)
            })
        }
    }
}

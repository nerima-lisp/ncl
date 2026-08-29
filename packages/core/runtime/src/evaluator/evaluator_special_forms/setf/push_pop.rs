use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_push(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("PUSH", "two", items.len().saturating_sub(1)));
        }

        let value = self.eval_in(&items[1], environment)?;
        let current = self.eval_in(&items[2], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| Self::invalid("PUSH place must contain a proper list", items[2].span))?;
        elements.insert(0, value);
        let result = Value::list(elements);
        self.set_place(&items[2], result.clone(), environment)?;
        Ok(result)
    }

    pub(crate) fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("POP", "one", items.len().saturating_sub(1)));
        }

        let current = self.eval_in(&items[1], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| Self::invalid("POP place must contain a proper list", items[1].span))?;
        let popped = if elements.is_empty() {
            Value::Nil
        } else {
            elements.remove(0)
        };
        self.set_place(&items[1], Value::list(elements), environment)?;
        Ok(popped)
    }
}

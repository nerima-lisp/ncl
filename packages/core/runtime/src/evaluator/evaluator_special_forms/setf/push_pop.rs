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

        let _guard = self.dynamic_guard();
        let value = self.eval_in(&items[1], environment)?;
        let (expansion, local) = self.capture_modify_place(&items[2], items, environment)?;
        let current = self.eval_in(&expansion.access_form, &local)?;
        self.store_modify_place(&expansion, &Value::cons(value, current), &local)
    }

    pub(crate) fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("POP", "one", items.len().saturating_sub(1)));
        }

        let _guard = self.dynamic_guard();
        let (expansion, local) = self.capture_modify_place(&items[1], items, environment)?;
        let current = self.eval_in(&expansion.access_form, &local)?;
        let (popped, tail) = match current {
            Value::Nil | Value::Boolean(false) => (Value::Nil, Value::Nil),
            Value::Cons(cell) => (cell.car(), cell.cdr()),
            _ => {
                return Err(Self::invalid(
                    "POP place must contain a list",
                    items[1].span,
                ));
            }
        };
        self.store_modify_place(&expansion, &tail, &local)?;
        Ok(popped)
    }
}

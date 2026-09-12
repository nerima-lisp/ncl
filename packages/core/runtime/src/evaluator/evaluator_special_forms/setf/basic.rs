use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len().is_multiple_of(2) {
            return Err(Self::invalid("setf needs place/value pairs", items[0].span));
        }
        self.execute_sequential_assignment(items, environment, |index| {
            self.eval_values_in(&items[2 + index * 2], environment)
        })
    }

    pub(crate) fn special_setf_intrinsic_store(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::invalid(
                "intrinsic SETF store needs a place and value",
                items[0].span,
            ));
        }
        let value = self.eval_in(&items[2], environment)?.primary_value();
        self.set_place(&items[1], value.clone(), environment)?;
        Ok(value)
    }

    pub(crate) fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "psetf needs place/value pairs",
                items[0].span,
            ));
        }

        self.execute_parallel_assignment(items, environment)
    }
}

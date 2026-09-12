use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.execute_place_transfer(items, environment, false)
    }

    pub(crate) fn special_shiftf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "SHIFTF",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        self.execute_place_transfer(items, environment, true)
    }
}

use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid("setf needs place/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].as_chunks::<2>().0 {
            let value = self.eval_in(&pair[1], environment)?;
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }

    pub(crate) fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "psetf needs place/value pairs",
                items[0].span,
            ));
        }

        let mut assignments = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].as_chunks::<2>().0 {
            let value = self.eval_in(&pair[1], environment)?;
            assignments.push((pair[0].clone(), value));
        }

        let mut result = Value::Nil;
        for (place, value) in assignments {
            self.set_place(&place, value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }
}

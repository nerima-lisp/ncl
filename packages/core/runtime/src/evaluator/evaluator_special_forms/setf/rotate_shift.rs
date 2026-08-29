use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let places = &items[1..];
        let values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() > 1 {
            let mut rotated = Vec::with_capacity(values.len());
            rotated.push(values.last().cloned().unwrap_or(Value::Nil));
            rotated.extend(values[..values.len() - 1].iter().cloned());
            for (place, value) in places.iter().zip(rotated) {
                self.set_place(place, value, environment)?;
            }
        }
        Ok(Value::Nil)
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

        let places = &items[1..items.len() - 1];
        let old_values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, place) in places.iter().enumerate() {
            let value = old_values
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| new_value.clone());
            self.set_place(place, value, environment)?;
        }
        Ok(old_values.into_iter().next().unwrap_or(Value::Nil))
    }
}

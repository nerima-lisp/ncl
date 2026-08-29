use super::{Environment, Form, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_mapcar(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "mapcar",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let function = self.eval_in(&items[1], environment)?;
        let sequences = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_list_mapping("MAPCAR", &function, &sequences, environment, items[0].span)
    }

    pub(crate) fn special_map_into(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "map-into",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let destination_form = &items[1];
        let destination = self.eval_in(destination_form, environment)?;
        let function = self.eval_in(&items[2], environment)?;
        let sequences = items[3..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.apply_sequence_map_into(
            &destination,
            &function,
            &sequences,
            environment,
            items[0].span,
        )?;
        self.set_map_into_destination(destination_form, result.clone(), environment)?;
        Ok(result)
    }
}

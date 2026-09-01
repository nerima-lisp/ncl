use super::{Environment, Form, Runtime, RuntimeError, Value, atom_name};

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
        let function = self.eval_in(&items[2], environment)?;
        let sequences = items[3..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if atom_name(destination_form).is_some() {
            let destination = self.eval_in(destination_form, environment)?;
            let result = self.apply_sequence_map_into(
                &destination,
                &function,
                &sequences,
                environment,
                items[0].span,
            )?;
            self.set_map_into_destination(destination_form, result.clone(), environment)?;
            return Ok(result);
        }

        let supports_setf_place = self.supports_setf_place(destination_form, environment);
        let expansion = match supports_setf_place.then(|| self.get_setf_expansion(destination_form, environment)) {
            Some(Ok(expansion)) => expansion,
            Some(Err(error)) => return Err(error),
            None => {
                let destination = self.eval_in(destination_form, environment)?;
                return self.apply_sequence_map_into(
                    &destination,
                    &function,
                    &sequences,
                    environment,
                    items[0].span,
                );
            }
        };
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(Self::invalid(
                "MAP-INTO SETF expansion temporary and value lists must have the same length",
                destination_form.span,
            ));
        }
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        let destination = self.eval_in(&expansion.access_form, &local)?;
        let result = self.apply_sequence_map_into(
            &destination,
            &function,
            &sequences,
            &local,
            items[0].span,
        )?;
        let (store_name, store_escaped) =
            Self::variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, result.clone(), &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(result)
    }
}

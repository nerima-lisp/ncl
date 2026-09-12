use super::{
    Environment, Form, FormKind, Runtime, RuntimeError, Value, normalize_name, unqualified_name,
};

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
        if let Some(result) =
            self.special_map_into_list_place(destination_form, items, environment)?
        {
            return Ok(result);
        }
        if let FormKind::List(destination_items) = &destination_form.kind
            && destination_items.len() == 3
            && matches!(&destination_items[0].kind, FormKind::Atom(name) if normalize_name(name) == "AREF")
            && matches!(destination_items[1].kind, FormKind::Atom(_))
        {
            let index = self.eval_in(&destination_items[2], environment)?;
            let container = self.eval_in(&destination_items[1], environment)?;
            let destination_index = Self::setf_index(index.clone(), destination_form.span)?;
            let destination = container
                .vector_items()
                .and_then(|items| items.get(destination_index).cloned())
                .ok_or_else(|| {
                    Self::invalid(
                        "MAP-INTO AREF index is out of bounds",
                        destination_form.span,
                    )
                })?;
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
            self.set_aref_vector_place_value(
                destination_form,
                &index,
                &container,
                result.clone(),
                environment,
            )?;
            return Ok(result);
        }
        if matches!(destination_form.kind, FormKind::List(_)) {
            if let Ok(expansion) = self.get_setf_expansion(destination_form, environment) {
                let local = environment.child();
                for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
                    let (name, escaped) =
                        Self::variable_name_info(temporary, "SETF temporary must be a symbol")?;
                    let value = self.eval_in(value_form, &local)?;
                    self.define_variable_in(&name, escaped, value, &local);
                }
                let destination = self.eval_in(&expansion.access_form, &local)?;
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
                match self.apply_setf_expansion(&expansion, &result, &local, destination_form.span)
                {
                    Err(RuntimeError::InvalidForm { message, .. })
                        if message == "unsupported SETF place" => {}
                    result => result?,
                }
                return Ok(result);
            }
        }
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

    fn special_map_into_list_place(
        &self,
        destination_form: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let FormKind::List(destination_items) = &destination_form.kind else {
            return Ok(None);
        };
        if destination_items.len() != 2 {
            return Ok(None);
        }
        let FormKind::Atom(operator_name) = &destination_items[0].kind else {
            return Ok(None);
        };
        if !matches!(
            unqualified_name(operator_name).as_str(),
            "CAR" | "CDR" | "FIRST" | "REST"
        ) || !matches!(destination_items[1].kind, FormKind::Atom(_))
        {
            return Ok(None);
        }
        let operator = unqualified_name(operator_name);
        let place_container = self.eval_in(&destination_items[1], environment)?;
        let destination = match operator.as_str() {
            "CAR" | "FIRST" => place_container
                .list_items()
                .and_then(|items| items.first().cloned())
                .ok_or_else(|| {
                    Self::invalid("cannot MAP-INTO CAR of NIL", destination_form.span)
                })?,
            "CDR" | "REST" => Value::list(
                place_container
                    .list_items()
                    .map(|items| items.iter().skip(1).cloned().collect())
                    .ok_or_else(|| {
                        Self::invalid("cannot MAP-INTO CDR of NIL", destination_form.span)
                    })?,
            ),
            _ => unreachable!(),
        };
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
        self.set_list_place_value(
            &operator,
            &destination_items[1],
            &place_container,
            result.clone(),
            environment,
            items[0].span,
        )?;
        Ok(Some(result))
    }
}

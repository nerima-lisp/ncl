use super::{Environment, OrdinaryLambdaList, Runtime, RuntimeError, Value};

pub(super) struct StructureBoaBindingContext<'a, F, D>
where
    F: Fn(&str) -> Option<usize>,
    D: Fn(&str) -> Result<Value, RuntimeError>,
{
    pub(super) lambda_list: &'a OrdinaryLambdaList,
    pub(super) arguments: &'a [Value],
    pub(super) required_count: usize,
    pub(super) optional_supplied_count: usize,
    pub(super) local: &'a Environment,
    pub(super) slot_index: &'a F,
    pub(super) evaluate_slot_default: &'a D,
    pub(super) slot_values: &'a mut [Option<Value>],
}

impl Runtime {
    pub(super) fn bind_structure_boa_required_optional<F, D>(
        &self,
        context: StructureBoaBindingContext<'_, F, D>,
    ) -> Result<(), RuntimeError>
    where
        F: Fn(&str) -> Option<usize>,
        D: Fn(&str) -> Result<Value, RuntimeError>,
    {
        let StructureBoaBindingContext {
            lambda_list,
            arguments,
            required_count,
            optional_supplied_count,
            local,
            slot_index,
            evaluate_slot_default,
            slot_values,
        } = context;
        for (index, (parameter, argument)) in lambda_list
            .required
            .iter()
            .zip(arguments.iter())
            .enumerate()
        {
            if lambda_list
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                self.define_exact_in(parameter, argument.clone(), local);
            } else {
                self.define_in(parameter, argument.clone(), local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), local);
            } else {
                self.define_in(&specification.name, value.clone(), local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }
}

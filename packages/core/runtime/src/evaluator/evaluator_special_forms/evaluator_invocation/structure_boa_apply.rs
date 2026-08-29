use super::{
    Environment, OrdinaryLambdaList, Runtime, RuntimeError, Span, StructureSlot, Value,
    structure_boa_binding_keywords::StructureBoaKeywordContext,
    structure_boa_binding_positional::StructureBoaBindingContext,
};

pub(super) struct StructureBoaConstructorContext<'a> {
    pub(super) name: &'a str,
    pub(super) slots: &'a [StructureSlot],
    pub(super) structure_types: &'a [String],
    pub(super) lambda_list: &'a OrdinaryLambdaList,
    pub(super) definition_environment: &'a Environment,
    pub(super) arguments: &'a [Value],
    pub(super) span: Span,
}

impl Runtime {
    pub(super) fn apply_structure_boa_constructor(
        &self,
        context: &StructureBoaConstructorContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureBoaConstructorContext {
            name,
            slots,
            structure_types,
            lambda_list,
            definition_environment,
            arguments,
            span,
        } = *context;
        let (required_count, optional_supplied_count, key_start) =
            Self::structure_boa_argument_counts(lambda_list, arguments)?;

        let local = definition_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut slot_values = vec![None; slots.len()];
        let slot_index =
            |parameter_name: &str| slots.iter().position(|slot| slot.name == parameter_name);
        let evaluate_slot_default = |parameter_name: &str| -> Result<Value, RuntimeError> {
            slots
                .iter()
                .find(|slot| slot.name == parameter_name)
                .and_then(|slot| slot.init_form.as_ref())
                .map(|form| self.eval_in(form, definition_environment))
                .transpose()
                .map(|value| value.unwrap_or(Value::Nil))
        };

        let binding_context = StructureBoaBindingContext {
            lambda_list,
            arguments,
            required_count,
            optional_supplied_count,
            local: &local,
            slot_index: &slot_index,
            evaluate_slot_default: &evaluate_slot_default,
            slot_values: &mut slot_values,
        };
        self.bind_structure_boa_required_optional(binding_context)?;

        if let Some(rest) = &lambda_list.rest {
            let value = Value::list(arguments[key_start..].to_vec());
            if lambda_list.rest_escaped {
                self.define_exact_in(rest, value.clone(), &local);
            } else {
                self.define_in(rest, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(rest) {
                slot_values[slot_index] = Some(value);
            }
        }

        if lambda_list.has_keyword_section {
            let keyword_context = StructureBoaKeywordContext {
                lambda_list,
                arguments,
                key_start,
                span,
                local: &local,
                slot_index: &slot_index,
                evaluate_slot_default: &evaluate_slot_default,
                slot_values: &mut slot_values,
            };
            self.bind_structure_boa_keywords(keyword_context)?;
        }

        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match slot_values[index].take() {
                Some(value) => value,
                None => evaluate_slot_default(&slot.name)?,
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }
}

use super::{
    Environment, OrdinaryLambdaList, Runtime, RuntimeError, Span, StructureSlot, Value,
    structure_boa_apply::StructureBoaConstructorContext,
};
use crate::environment::names_equal;

pub(super) struct StructureConstructorContext<'a> {
    pub(super) name: &'a str,
    pub(super) slots: &'a [StructureSlot],
    pub(super) structure_types: &'a [String],
    pub(super) constructor_lambda_list: Option<&'a OrdinaryLambdaList>,
    pub(super) definition_environment: &'a Environment,
    pub(super) arguments: &'a [Value],
    pub(super) span: Span,
}

impl Runtime {
    pub(super) fn apply_structure_constructor(
        &self,
        context: &StructureConstructorContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureConstructorContext {
            name,
            slots,
            structure_types,
            constructor_lambda_list,
            definition_environment,
            arguments,
            span,
        } = *context;
        if let Some(lambda_list) = constructor_lambda_list {
            return self.apply_structure_boa_constructor(&StructureBoaConstructorContext {
                name,
                slots,
                structure_types,
                lambda_list,
                definition_environment,
                arguments,
                span,
            });
        }
        if !arguments.len().is_multiple_of(2) {
            return Err(Self::arity(
                "structure constructor",
                "an even number of",
                arguments.len(),
            ));
        }
        let mut supplied = vec![None; slots.len()];
        for pair in arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword_name) | Value::KeywordExact(keyword_name)) = &pair[0]
            else {
                return Err(Self::invalid(
                    "structure constructor keyword name must be a keyword",
                    span,
                ));
            };
            let Some(index) = slots
                .iter()
                .position(|slot| names_equal(&slot.name, keyword_name))
            else {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown structure keyword :{keyword_name}"),
                    span: Some(span),
                });
            };
            supplied[index] = Some(pair[1].clone());
        }
        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match supplied[index].clone() {
                Some(value) => value,
                None => slot
                    .init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, definition_environment))
                    .transpose()?
                    .unwrap_or(Value::Nil),
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }

    pub(super) fn apply_structure_predicate(
        name: &str,
        arguments: &[Value],
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure predicate", "one", arguments.len()));
        }
        Ok(Value::boolean(arguments[0].structure_is_type(name)))
    }

    pub(super) fn apply_structure_accessor(
        structure_name: &str,
        slot_index: usize,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure accessor", "one", arguments.len()));
        }
        if !arguments[0].structure_is_type(structure_name) {
            return Err(RuntimeError::Type {
                expected: structure_name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        arguments[0]
            .structure_slot(slot_index)
            .ok_or_else(|| Self::invalid("structure slot is out of range", span))
    }

    pub(super) fn apply_structure_copier(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure copier", "one", arguments.len()));
        }
        if !arguments[0].structure_is_type(name) {
            return Err(RuntimeError::Type {
                expected: name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        arguments[0]
            .copy_structure()
            .ok_or_else(|| Self::invalid("structure copy failed", span))
    }
}

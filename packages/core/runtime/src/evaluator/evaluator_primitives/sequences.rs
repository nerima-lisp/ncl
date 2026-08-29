#![allow(clippy::wildcard_imports)]
use super::evaluator_special_forms::evaluator_sequences::sequence_types::SequenceSubstituteContext;
use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        self.apply_sequence_mutation_primitive(name, arguments, environment, span)
            .or_else(|| self.apply_sequence_set_primitive(name, arguments, environment, span))
            .or_else(|| self.apply_sequence_search_primitive(name, arguments, environment, span))
    }

    fn apply_sequence_mutation_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "REMOVE" | "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE" | "DELETE-IF" | "DELETE-IF-NOT"
                if arguments.len() >= 2 =>
            {
                self.apply_sequence_remove(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" if !arguments.is_empty() => self
                .apply_sequence_remove(
                    name,
                    &Value::Nil,
                    &arguments[0],
                    &arguments[1..],
                    environment,
                    span,
                ),
            "REMOVE-DUPLICATES" | "DELETE-DUPLICATES" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least one",
                arguments.len(),
            )),
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT"
                if arguments.len() >= 3 =>
            {
                self.apply_sequence_substitute(SequenceSubstituteContext {
                    operation: name,
                    new_item: &arguments[0],
                    old_or_predicate: &arguments[1],
                    sequence: &arguments[2],
                    options: &arguments[3..],
                    environment,
                    span,
                })
            }
            "SUBSTITUTE" | "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE"
            | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least three",
                arguments.len(),
            )),
            _ => return None,
        })
    }
}

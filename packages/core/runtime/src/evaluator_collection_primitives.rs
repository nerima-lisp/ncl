#[allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn apply_sequence_collection(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match name {
            "MAP" => {
                if arguments.len() < 3 {
                    return Err(Self::arity("map", "at least three", arguments.len()));
                }
                self.apply_sequence_mapping(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "REDUCE" => {
                if arguments.len() < 2 {
                    return Err(Self::arity("reduce", "at least two", arguments.len()));
                }
                self.apply_sequence_reduce(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "MERGE" => {
                if arguments.len() < 4 {
                    return Err(Self::arity("merge", "at least four", arguments.len()));
                }
                self.apply_sequence_merge(SequenceMergeContext {
                    result_type: &arguments[0],
                    sequence1: &arguments[1],
                    sequence2: &arguments[2],
                    predicate: &arguments[3],
                    options: &arguments[4..],
                    environment,
                    span,
                })
            }
            "MAP-INTO" => {
                if arguments.len() < 2 {
                    return Err(Self::arity("map-into", "at least two", arguments.len()));
                }
                self.apply_sequence_map_into(
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            _ => Err(Self::invalid("unknown sequence collection primitive", span)),
        }
    }
}

#![allow(clippy::wildcard_imports)]
use super::*;

struct ListMappingMode {
    uses_tails: bool,
    concatenates: bool,
    returns_first: bool,
}

impl Runtime {
    pub(crate) fn apply_list_mapping(
        &self,
        operation: &str,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let mode = match operation {
            "MAPC" => ListMappingMode {
                uses_tails: false,
                concatenates: false,
                returns_first: true,
            },
            "MAPCAR" => ListMappingMode {
                uses_tails: false,
                concatenates: false,
                returns_first: false,
            },
            "MAPL" => ListMappingMode {
                uses_tails: true,
                concatenates: false,
                returns_first: true,
            },
            "MAPLIST" => ListMappingMode {
                uses_tails: true,
                concatenates: false,
                returns_first: false,
            },
            "MAPCAN" => ListMappingMode {
                uses_tails: false,
                concatenates: true,
                returns_first: false,
            },
            "MAPCON" => ListMappingMode {
                uses_tails: true,
                concatenates: true,
                returns_first: false,
            },
            _ => return Err(Self::invalid("unknown list mapping operation", span)),
        };
        let operation_name = operation.to_ascii_lowercase();
        let lists = sequences
            .iter()
            .map(|value| {
                value.list_cells().ok_or_else(|| {
                    Self::invalid(
                        &format!("{operation_name} arguments must be proper lists"),
                        span,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = lists.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = if mode.uses_tails {
                lists
                    .iter()
                    .map(|items| Value::Cons(items[index].clone()))
                    .collect::<Vec<_>>()
            } else {
                lists
                    .iter()
                    .map(|items| items[index].car())
                    .collect::<Vec<_>>()
            };
            let result = self
                .apply_in(function, &arguments, span, environment)?
                .primary_value();
            if !mode.returns_first {
                results.push(result);
            }
        }
        if mode.returns_first {
            Ok(sequences.first().cloned().unwrap_or(Value::Nil))
        } else if mode.concatenates {
            crate::builtins::nconc_lists(&operation_name, &results).map_err(|_| {
                Self::invalid(
                    &format!(
                        "{operation_name} nonfinal function results must be noncircular lists"
                    ),
                    span,
                )
            })
        } else {
            Ok(Value::list(results))
        }
    }
}

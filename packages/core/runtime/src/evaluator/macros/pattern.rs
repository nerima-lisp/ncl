use ncl_syntax::Span;

use crate::evaluator::helpers::macro_dotted_parts;
use crate::value::MacroPattern;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn bind_macro_pattern(
        pattern: &MacroPattern,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            MacroPattern::Name(name) => {
                environment.define(name, value);
                Ok(())
            }
            MacroPattern::List(patterns) => {
                let Some(values) = value.list_items() else {
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a proper list",
                        span,
                    ));
                };
                if values.len() != patterns.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a list",
                        span,
                    ));
                };
                if values.len() < items.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has too few elements",
                        span,
                    ));
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                Self::bind_macro_pattern(tail, tail_value, environment, span)
            }
        }
    }
}

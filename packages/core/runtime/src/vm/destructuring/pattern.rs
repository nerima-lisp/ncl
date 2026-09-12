use ncl_compiler::DestructurePattern;
use ncl_syntax::Span;

use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(in crate::vm) fn destructure_value(
    pattern: &DestructurePattern,
    value: Value,
    runtime: &Runtime,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    match pattern {
        DestructurePattern::Name(name) => {
            runtime.define_in(name, value, environment);
            Ok(())
        }
        DestructurePattern::List(patterns) => {
            let Some(values) = value.list_items() else {
                return Err(invalid(
                    "destructuring-bind pattern requires a proper list",
                    span,
                ));
            };
            if values.len() != patterns.len() {
                return Err(invalid(
                    "destructuring-bind pattern has the wrong number of elements",
                    span,
                ));
            }
            for (pattern, value) in patterns.iter().zip(values) {
                destructure_value(pattern, value, runtime, environment, span)?;
            }
            Ok(())
        }
        DestructurePattern::Dotted { items, tail } => {
            let Some((values, dotted_tail)) = destructure_dotted_parts(&value) else {
                return Err(invalid("destructuring-bind pattern requires a list", span));
            };
            if values.len() < items.len() {
                return Err(invalid(
                    "destructuring-bind pattern has too few elements",
                    span,
                ));
            }
            for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                destructure_value(pattern, value, runtime, environment, span)?;
            }
            let remaining = values[items.len()..].to_vec();
            let tail_value = if remaining.is_empty() {
                dotted_tail
            } else if dotted_tail.is_truthy() {
                Value::dotted_list(remaining, dotted_tail)
            } else {
                Value::list(remaining)
            };
            destructure_value(tail, tail_value, runtime, environment, span)
        }
    }
}

pub(in crate::vm) fn destructure_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    value.list_parts()
}

use std::collections::{HashMap, HashSet};

use crate::value::SharedCons;

use super::*;

impl Runtime {
    pub(super) fn apply_list_topology_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "LDIFF" if arguments.len() == 2 => Self::apply_ldiff(arguments, span),
            "LDIFF" => Err(Self::arity("ldiff", "two", arguments.len())),
            "TAILP" if arguments.len() == 2 => Self::apply_tailp(arguments, span),
            "TAILP" => Err(Self::arity("tailp", "two", arguments.len())),
            _ => return None,
        })
    }

    fn apply_ldiff(arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        let list = &arguments[0];
        if !matches!(list, Value::Nil | Value::Boolean(false) | Value::Cons(_)) {
            return Err(RuntimeError::Type {
                expected: "LIST".to_owned(),
                actual: list.type_name().to_owned(),
                span: Some(span),
            });
        }

        let target = &arguments[1];
        let target_is_nil = matches!(target, Value::Nil | Value::Boolean(false));
        let mut current = list.clone();
        let mut result = Value::Nil;
        let mut last: Option<SharedCons> = None;
        let mut copies: HashMap<usize, Value> = HashMap::new();

        loop {
            if current.eq_value(target)
                || (target_is_nil && matches!(current, Value::Nil | Value::Boolean(false)))
            {
                return Ok(result);
            }

            let Value::Cons(cell) = current else {
                if !matches!(current, Value::Nil | Value::Boolean(false)) {
                    if let Some(last) = &last {
                        last.set_cdr(current.clone());
                    }
                }
                return Ok(result);
            };
            if let Some(existing) = copies.get(&cell.identity()) {
                if let Some(last) = &last {
                    last.set_cdr(existing.clone());
                }
                return Ok(result);
            }

            let copied = Value::cons(cell.car(), Value::Nil);
            let Value::Cons(copied_cell) = copied.clone() else {
                unreachable!();
            };
            if let Some(last) = &last {
                last.set_cdr(copied.clone());
            } else {
                result = copied.clone();
            }
            last = Some(copied_cell);
            copies.insert(cell.identity(), copied);
            current = cell.cdr();
        }
    }

    fn apply_tailp(arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        let list = &arguments[1];
        if !matches!(list, Value::Nil | Value::Boolean(false) | Value::Cons(_)) {
            return Err(RuntimeError::Type {
                expected: "LIST".to_owned(),
                actual: list.type_name().to_owned(),
                span: Some(span),
            });
        }

        let target = &arguments[0];
        let mut current = list.clone();
        let mut seen = HashSet::new();
        loop {
            if current.eq_value(target) {
                return Ok(Value::Boolean(true));
            }
            let Value::Cons(cell) = current else {
                return Ok(Value::Nil);
            };
            if !seen.insert(cell.identity()) {
                return Ok(Value::Nil);
            }
            current = cell.cdr();
        }
    }
}

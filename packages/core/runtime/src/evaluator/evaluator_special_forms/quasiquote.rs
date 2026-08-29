#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_quasiquote(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "quasiquote",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        self.quasiquote_value(&items[1], environment)
    }

    pub(crate) fn quasiquote_value(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.quasiquote_value_at(form, environment, 1)
    }

    pub(super) fn quasiquote_items(
        &self,
        items: &[Form],
        environment: &Environment,
        depth: usize,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut values = Vec::with_capacity(items.len());
        for item in items {
            if depth == 1
                && let Some(argument) = prefix_argument(
                    match &item.kind {
                        FormKind::List(items) => items,
                        _ => &[],
                    },
                    "UNQUOTE-SPLICING",
                )
            {
                values.extend(self.quasiquote_splice(argument, environment, item.span)?);
                continue;
            }
            values.push(self.quasiquote_value_at(item, environment, depth)?);
        }
        Ok(values)
    }

    pub(super) fn quasiquote_value_at(
        &self,
        form: &Form,
        environment: &Environment,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) | FormKind::String(_) | FormKind::Character(_) => {
                Self::quoted_value(form)
            }
            FormKind::Vector(items) => {
                let values = self.quasiquote_items(items, environment, depth)?;
                Ok(Value::vector(values))
            }
            FormKind::List(items) => {
                if let Some(argument) = prefix_argument(items, "UNQUOTE") {
                    if depth == 1 {
                        return self.eval_in(argument, environment);
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE",
                        self.quasiquote_value_at(argument, environment, depth - 1)?,
                    ));
                }
                if let Some(item) = prefix_argument(items, "UNQUOTE-SPLICING") {
                    if depth == 1 {
                        return Err(Self::invalid(
                            "unquote-splicing is only valid inside a list or vector",
                            item.span,
                        ));
                    }
                    return Ok(quasiquote_marker(
                        "UNQUOTE-SPLICING",
                        self.quasiquote_value_at(item, environment, depth - 1)?,
                    ));
                }
                if let Some(argument) = prefix_argument(items, "QUASIQUOTE") {
                    return Ok(quasiquote_marker(
                        "QUASIQUOTE",
                        self.quasiquote_value_at(argument, environment, depth + 1)?,
                    ));
                }
                let values = self.quasiquote_items(items, environment, depth)?;
                Ok(Value::list(values))
            }
            FormKind::DottedList { items, tail } => {
                let mut values = self.quasiquote_items(items, environment, depth)?;
                if let Some(argument) = prefix_argument(
                    match &tail.kind {
                        FormKind::List(items) => items,
                        _ => &[],
                    },
                    "UNQUOTE-SPLICING",
                ) && depth == 1
                {
                    let mut spliced = self.quasiquote_splice(argument, environment, tail.span)?;
                    values.append(&mut spliced);
                    return Ok(Value::list(values));
                }
                let tail_value = self.quasiquote_value_at(tail, environment, depth)?;
                if depth == 1
                    && let Some(mut tail_items) = tail_value.list_items()
                {
                    values.append(&mut tail_items);
                    return Ok(Value::list(values));
                }
                Ok(Value::dotted_list(values, tail_value))
            }
        }
    }

    pub(super) fn quasiquote_splice(
        &self,
        argument: &Form,
        environment: &Environment,
        span: Span,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = self.eval_in(argument, environment)?;
        value
            .list_items()
            .ok_or_else(|| Self::invalid("unquote-splicing requires a proper list", span))
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn quasiquote_propagates_errors_from_nested_unquotes_splices_and_vectors() {
        for source in [
            "(quasiquote (a (quasiquote (unquote (unquote (car 5))))))",
            "(quasiquote #((unquote (car 5))))",
            "(quasiquote ((unquote (car 5)) . b))",
            "(quasiquote (a . (unquote-splicing (car 5))))",
            "(quasiquote (1 (unquote-splicing (car 5))))",
            "(quasiquote (a . (unquote (car 5))))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn quasiquote_rejects_unquote_splicing_a_non_list_value() {
        let error = Runtime::new()
            .eval_source("(quasiquote (1 (unquote-splicing 5)))")
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            error,
            crate::RuntimeError::InvalidForm { message, .. }
                if message == "unquote-splicing requires a proper list"
        ));
    }
}

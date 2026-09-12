use super::{Environment, Form, FormKind, Runtime, RuntimeError, prefix_argument};

impl Runtime {
    pub(super) fn prepare_compiled_quasiquote(
        &self,
        form: &Form,
        environment: &Environment,
        depth: usize,
    ) -> Result<Form, RuntimeError> {
        if let FormKind::List(items) = &form.kind {
            for operator in ["UNQUOTE", "UNQUOTE-SPLICING", "QUASIQUOTE"] {
                if let Some(argument) = prefix_argument(items, operator) {
                    let prepared = if operator == "QUASIQUOTE" {
                        self.prepare_compiled_quasiquote(argument, environment, depth + 1)?
                    } else if depth == 1 {
                        self.prepare_compiled_form(argument, environment)?
                    } else {
                        self.prepare_compiled_quasiquote(argument, environment, depth - 1)?
                    };
                    return Ok(Form::list(vec![items[0].clone(), prepared], form.span));
                }
            }
        }
        let prepare_items = |items: &[Form]| {
            items
                .iter()
                .map(|item| self.prepare_compiled_quasiquote(item, environment, depth))
                .collect::<Result<Vec<_>, _>>()
        };
        let kind = match &form.kind {
            FormKind::List(items) => FormKind::List(prepare_items(items)?),
            FormKind::Vector(items) => FormKind::Vector(prepare_items(items)?),
            FormKind::DottedList { items, tail } => FormKind::DottedList {
                items: prepare_items(items)?,
                tail: Box::new(self.prepare_compiled_quasiquote(tail, environment, depth)?),
            },
            _ => return Ok(form.clone()),
        };
        Ok(Form::new(kind, form.span))
    }
}

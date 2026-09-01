use ncl_syntax::{Form, FormKind};

use crate::{environment::names_equal, evaluator::helpers::atom_name, Runtime, RuntimeError};

impl Runtime {
    pub(super) fn expand_builtin_loop(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let mut body = items[1..].to_vec();
        if let Some(clause) = items.get(1).and_then(atom_name) {
            if names_equal(clause, "WHILE") || names_equal(clause, "UNTIL") {
                if items.len() < 3 {
                    return Err(Self::invalid("LOOP condition clause requires a test", form.span));
                }
                let stop_on_true = names_equal(clause, "UNTIL");
                let body_start = usize::from(
                    items
                        .get(3)
                        .and_then(atom_name)
                        .is_some_and(|name| names_equal(name, "DO")),
                ) + 3;
                let return_form = Form::list(
                    vec![Form::atom("RETURN", form.span)],
                    form.span,
                );
                let guard_operator = if stop_on_true { "WHEN" } else { "UNLESS" };
                body = vec![Form::list(
                    vec![
                        Form::atom(guard_operator, form.span),
                        items[2].clone(),
                        return_form,
                    ],
                    form.span,
                )];
                body.extend(items[body_start..].iter().cloned());
            }
        }

        let tag = Form::atom(format!("#:NCL-LOOP-{}", form.span.start), form.span);
        let mut tagbody = vec![Form::atom("TAGBODY", form.span), tag.clone()];
        tagbody.extend(body);
        tagbody.push(Form::list(
            vec![Form::atom("GO", form.span), tag],
            form.span,
        ));

        Ok(Form::list(
            vec![
                Form::atom("BLOCK", form.span),
                Form::atom("NIL", form.span),
                Form::list(tagbody, form.span),
            ],
            form.span,
        ))
    }
}

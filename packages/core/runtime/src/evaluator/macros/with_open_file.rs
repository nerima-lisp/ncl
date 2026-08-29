use ncl_syntax::{Form, FormKind};

use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(in crate::evaluator) fn expand_with_open_file(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(Self::arity(
                "with-open-file",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(Self::invalid(
                "with-open-file binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(Self::invalid(
                "with-open-file binding needs a stream variable and pathname",
                binding_form.span,
            ));
        }
        Self::variable_name_info(
            &binding[0],
            "with-open-file stream variable must be a symbol",
        )?;

        let mut open_items = Vec::with_capacity(binding.len());
        open_items.push(Form::atom("OPEN", binding_form.span));
        open_items.extend(binding[1..].iter().cloned());
        let open_form = Form::list(open_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), open_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, Span};

    use crate::Runtime;

    const SPAN: Span = Span::new(0, 1);

    fn atom(name: &str) -> Form {
        Form::atom(name, SPAN)
    }

    fn valid(result: Result<Form, crate::RuntimeError>) -> Form {
        result.unwrap_or_else(|error| panic!("expected a successful expansion: {error}"))
    }

    #[test]
    fn a_non_list_form_passes_through_unchanged() {
        let form = atom("X");
        let expanded = valid(Runtime::expand_with_open_file(&form));
        assert_eq!(expanded.to_string(), form.to_string());
    }

    #[test]
    fn rejects_a_form_with_no_binding() {
        let form = Form::list(vec![atom("WITH-OPEN-FILE")], SPAN);
        assert!(Runtime::expand_with_open_file(&form).is_err());
    }

    #[test]
    fn rejects_a_non_list_binding() {
        let form = Form::list(vec![atom("WITH-OPEN-FILE"), atom("S")], SPAN);
        assert!(Runtime::expand_with_open_file(&form).is_err());
    }

    #[test]
    fn rejects_a_binding_missing_a_pathname() {
        let form = Form::list(
            vec![atom("WITH-OPEN-FILE"), Form::list(vec![atom("S")], SPAN)],
            SPAN,
        );
        assert!(Runtime::expand_with_open_file(&form).is_err());
    }

    #[test]
    fn rejects_a_non_symbol_stream_variable() {
        let form = Form::list(
            vec![
                atom("WITH-OPEN-FILE"),
                Form::list(vec![atom("5"), atom("FILE")], SPAN),
            ],
            SPAN,
        );
        assert!(Runtime::expand_with_open_file(&form).is_err());
    }

    #[test]
    fn a_binding_without_a_body_expands_to_a_nil_body() {
        let form = Form::list(
            vec![
                atom("WITH-OPEN-FILE"),
                Form::list(vec![atom("S"), atom("FILE")], SPAN),
            ],
            SPAN,
        );
        let expanded = valid(Runtime::expand_with_open_file(&form));
        assert!(expanded.to_string().contains("UNWIND-PROTECT NIL"));
    }
}

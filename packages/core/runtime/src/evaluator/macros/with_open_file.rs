use ncl_syntax::{Form, FormKind};

use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(in crate::evaluator) fn expand_with_open_file(form: &Form) -> Result<Form, RuntimeError> {
        Self::expand_with_open(form, true, "with-open-file")
    }

    pub(in crate::evaluator) fn expand_with_open_stream(form: &Form) -> Result<Form, RuntimeError> {
        Self::expand_with_open(form, false, "with-open-stream")
    }

    fn expand_with_open(
        form: &Form,
        open_pathnames: bool,
        operator: &str,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(Self::arity(
                operator,
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(Self::invalid(
                "with-open binding must be a list",
                binding_form.span,
            ));
        };
        if bindings.is_empty() {
            return Err(Self::invalid(
                "with-open needs at least one binding",
                binding_form.span,
            ));
        }
        let mut generated_bindings = Vec::with_capacity(bindings.len());
        let mut stream_names = Vec::with_capacity(bindings.len());
        for binding_form in bindings {
            let FormKind::List(binding) = &binding_form.kind else {
                return Err(Self::invalid(
                    "with-open binding must be a list",
                    binding_form.span,
                ));
            };
            if binding.len() < 2 {
                return Err(Self::invalid(
                    "with-open binding needs a stream variable and stream form",
                    binding_form.span,
                ));
            }
            Self::variable_name_info(
                &binding[0],
                "with-open stream variable must be a symbol",
            )?;
            let open_items = if open_pathnames {
                let mut items = Vec::with_capacity(binding.len());
                items.push(Form::atom("OPEN", binding_form.span));
                items.extend(binding[1..].iter().cloned());
                Form::list(items, binding_form.span)
            } else {
                binding[1].clone()
            };
            generated_bindings.push(Form::list(
                vec![
                    binding[0].clone(),
                    open_items,
                ],
                binding_form.span,
            ));
            stream_names.push(binding[0].clone());
        }
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let protected_form = stream_names.into_iter().rev().fold(body, |body, stream| {
            let close_form = Form::list(
                vec![Form::atom("CLOSE", form.span), stream],
                form.span,
            );
            Form::list(
                vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
                form.span,
            )
        });
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(generated_bindings, binding_form.span),
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
                Form::list(
                    vec![Form::list(vec![atom("S"), atom("FILE")], SPAN)],
                    SPAN,
                ),
            ],
            SPAN,
        );
        let expanded = valid(Runtime::expand_with_open_file(&form));
        assert!(expanded.to_string().contains("UNWIND-PROTECT NIL"));
    }

    #[test]
    fn accepts_multiple_bindings_and_closes_them_in_reverse_order() {
        let form = Form::list(
            vec![
                atom("WITH-OPEN-FILE"),
                Form::list(
                    vec![
                        Form::list(vec![atom("S"), atom("FIRST")], SPAN),
                        Form::list(vec![atom("U"), atom("SECOND")], SPAN),
                    ],
                    SPAN,
                ),
                atom("S"),
            ],
            SPAN,
        );
        let expanded = valid(Runtime::expand_with_open_file(&form));
        assert_eq!(
            expanded.to_string(),
            "(LET ((S (OPEN FIRST)) (U (OPEN SECOND))) (UNWIND-PROTECT (UNWIND-PROTECT (PROGN S) (CLOSE U)) (CLOSE S)))"
        );
    }

    #[test]
    fn with_open_stream_uses_the_supplied_stream_form() {
        let form = Form::list(
            vec![
                atom("WITH-OPEN-STREAM"),
                Form::list(vec![Form::list(vec![atom("S"), atom("SOURCE")], SPAN)], SPAN),
                atom("S"),
            ],
            SPAN,
        );
        let expanded = valid(Runtime::expand_with_open_stream(&form));
        assert_eq!(
            expanded.to_string(),
            "(LET ((S SOURCE)) (UNWIND-PROTECT (PROGN S) (CLOSE S)))"
        );
    }
}

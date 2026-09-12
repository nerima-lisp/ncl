use ncl_syntax::{Form, FormKind};

use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(in crate::evaluator) fn expand_with_open_stream(
        &self,
        form: &Form,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(Self::arity(
                "with-open-stream",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "with-open-stream binding must be a list",
                items[1].span,
            ));
        };
        if bindings.is_empty() {
            return Err(Self::invalid(
                "with-open-stream needs at least one binding",
                items[1].span,
            ));
        }
        let mut generated_bindings = Vec::with_capacity(bindings.len() + 1);
        let mut stream_names = Vec::with_capacity(bindings.len());
        let mut flags = Vec::with_capacity(bindings.len());
        for binding_form in bindings {
            let FormKind::List(binding) = &binding_form.kind else {
                return Err(Self::invalid(
                    "with-open-stream binding must be a list",
                    binding_form.span,
                ));
            };
            if binding.len() != 2 {
                return Err(Self::invalid(
                    "with-open-stream binding needs a stream variable and stream",
                    binding_form.span,
                ));
            }
            Self::variable_name_info(
                &binding[0],
                "with-open-stream stream variable must be a symbol",
            )?;
            generated_bindings.push(Form::list(
                vec![binding[0].clone(), binding[1].clone()],
                binding_form.span,
            ));
            stream_names.push(binding[0].clone());
            flags.push(self.fresh_with_open_file_flag(form));
        }
        generated_bindings.extend(flags.iter().map(|flag| {
            Form::list(
                vec![flag.clone(), Form::atom("NIL", form.span)],
                items[1].span,
            )
        }));
        let body = if items.len() > 2 {
            let mut body_items = vec![Form::atom("PROGN", form.span)];
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let mut protected = body;
        for (stream, flag) in stream_names.into_iter().zip(flags).rev() {
            let mark = Form::list(
                vec![
                    Form::atom("SETQ", form.span),
                    flag.clone(),
                    Form::atom("T", form.span),
                ],
                form.span,
            );
            let close = Form::list(vec![Form::atom("CLOSE", form.span), stream], form.span);
            protected = Form::list(
                vec![
                    Form::atom("UNWIND-PROTECT", form.span),
                    Form::list(
                        vec![
                            Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                            protected,
                            mark,
                        ],
                        form.span,
                    ),
                    close,
                ],
                form.span,
            );
        }
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(generated_bindings, items[1].span),
                protected,
            ],
            form.span,
        ))
    }

    pub(in crate::evaluator) fn expand_with_open_file(
        &self,
        form: &Form,
    ) -> Result<Form, RuntimeError> {
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
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(Self::invalid(
                "with-open-file binding must be a list",
                binding_form.span,
            ));
        };
        if bindings.is_empty() {
            return Err(Self::invalid(
                "with-open-file needs at least one binding",
                binding_form.span,
            ));
        }
        let binding_specs = if matches!(bindings[0].kind, FormKind::List(_)) {
            bindings
                .iter()
                .map(|binding_form| {
                    let FormKind::List(binding) = &binding_form.kind else {
                        return Err(Self::invalid(
                            "with-open-file binding must be a list",
                            binding_form.span,
                        ));
                    };
                    Ok((binding.as_slice(), binding_form.span))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![(bindings.as_slice(), binding_form.span)]
        };
        let mut generated_bindings = Vec::with_capacity(binding_specs.len() + 1);
        let mut stream_names = Vec::with_capacity(binding_specs.len());
        let mut flags = Vec::with_capacity(binding_specs.len());
        for (binding, binding_span) in binding_specs {
            if binding.len() < 2 {
                return Err(Self::invalid(
                    "with-open-file binding needs a stream variable and pathname",
                    binding_span,
                ));
            }
            Self::variable_name_info(
                &binding[0],
                "with-open-file stream variable must be a symbol",
            )?;
            let mut open_items = vec![Form::atom("OPEN", binding_span)];
            open_items.extend(binding[1..].iter().cloned());
            generated_bindings.push(Form::list(
                vec![binding[0].clone(), Form::list(open_items, binding_span)],
                binding_span,
            ));
            stream_names.push(binding[0].clone());
            flags.push(self.fresh_with_open_file_flag(form));
        }
        generated_bindings.extend(flags.iter().map(|flag| {
            Form::list(
                vec![flag.clone(), Form::atom("NIL", form.span)],
                binding_form.span,
            )
        }));
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let mut protected_form = body;
        for (stream, flag) in stream_names.into_iter().zip(flags).rev() {
            let mark_normal_form = Form::list(
                vec![
                    Form::atom("SETQ", form.span),
                    flag.clone(),
                    Form::atom("T", form.span),
                ],
                form.span,
            );
            let result_form = Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    protected_form,
                    mark_normal_form,
                ],
                form.span,
            );
            let abort_form = Form::list(vec![Form::atom("NOT", form.span), flag], form.span);
            let close_form = Form::list(
                vec![
                    Form::atom("CLOSE", form.span),
                    stream,
                    Form::atom(":ABORT", form.span),
                    abort_form,
                ],
                form.span,
            );
            protected_form = Form::list(
                vec![
                    Form::atom("UNWIND-PROTECT", form.span),
                    result_form,
                    close_form,
                ],
                form.span,
            );
        }
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(generated_bindings, binding_form.span),
                protected_form,
            ],
            form.span,
        ))
    }

    fn fresh_with_open_file_flag(&self, form: &Form) -> Form {
        loop {
            let counter = self.gensym_counter.get();
            self.gensym_counter.set(counter.wrapping_add(1));
            let name = format!("NCL-WITH-OPEN-FILE-NORMAL-{counter}");
            if !form_contains_symbol(form, &name) {
                return Form::atom(name, form.span);
            }
        }
    }
}

fn form_contains_symbol(form: &Form, name: &str) -> bool {
    match &form.kind {
        FormKind::Atom(atom) => atom.eq_ignore_ascii_case(name),
        FormKind::List(items) | FormKind::Vector(items) => {
            items.iter().any(|item| form_contains_symbol(item, name))
        }
        FormKind::DottedList { items, tail } => {
            items.iter().any(|item| form_contains_symbol(item, name))
                || form_contains_symbol(tail, name)
        }
        FormKind::Complex { real, imaginary } => {
            form_contains_symbol(real, name) || form_contains_symbol(imaginary, name)
        }
        FormKind::String(_)
        | FormKind::Character(_)
        | FormKind::Literal(_)
        | FormKind::CircularReference => false,
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
        let runtime = Runtime::new();
        let expanded = valid(runtime.expand_with_open_file(&form));
        assert_eq!(expanded.to_string(), form.to_string());
    }

    #[test]
    fn rejects_a_form_with_no_binding() {
        let form = Form::list(vec![atom("WITH-OPEN-FILE")], SPAN);
        assert!(Runtime::new().expand_with_open_file(&form).is_err());
    }

    #[test]
    fn rejects_a_non_list_binding() {
        let form = Form::list(vec![atom("WITH-OPEN-FILE"), atom("S")], SPAN);
        assert!(Runtime::new().expand_with_open_file(&form).is_err());
    }

    #[test]
    fn rejects_a_binding_missing_a_pathname() {
        let form = Form::list(
            vec![atom("WITH-OPEN-FILE"), Form::list(vec![atom("S")], SPAN)],
            SPAN,
        );
        assert!(Runtime::new().expand_with_open_file(&form).is_err());
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
        assert!(Runtime::new().expand_with_open_file(&form).is_err());
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
        let runtime = Runtime::new();
        let expanded = valid(runtime.expand_with_open_file(&form));
        assert!(expanded.to_string().contains("MULTIPLE-VALUE-PROG1 NIL"));
    }
}

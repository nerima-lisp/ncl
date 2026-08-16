impl Runtime {
    fn expand_with_open_file(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-open-file",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid("with-open-file binding must be a list", binding_form.span));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-open-file binding needs a stream variable and pathname",
                binding_form.span,
            ));
        }
        self.variable_name_info(
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

    fn expand_with_output_to_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-output-to-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-output-to-string binding must be a list",
                binding_form.span,
            ));
        };
        if !(1..=2).contains(&binding.len()) {
            return Err(self.invalid(
                "with-output-to-string binding needs a stream variable and optional string place",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-output-to-string stream variable must be a symbol",
        )?;

        let output_form = Form::list(
            vec![Form::atom("MAKE-STRING-OUTPUT-STREAM", binding_form.span)],
            binding_form.span,
        );
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), output_form],
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
        let output_string_form = Form::list(
            vec![
                Form::atom("GET-OUTPUT-STREAM-STRING", form.span),
                binding[0].clone(),
            ],
            form.span,
        );
        let result_form = if let Some(string_place) = binding.get(1) {
            let append_form = Form::list(
                vec![
                    Form::atom("__NCL_APPEND_OUTPUT_TO_STRING", form.span),
                    string_place.clone(),
                    output_string_form,
                ],
                form.span,
            );
            let setf_form = Form::list(
                vec![
                    Form::atom("SETF", form.span),
                    string_place.clone(),
                    append_form,
                ],
                form.span,
            );
            Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    body,
                    setf_form,
                ],
                form.span,
            )
        } else {
            Form::list(
                vec![Form::atom("PROGN", form.span), body, output_string_form],
                form.span,
            )
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![
                Form::atom("UNWIND-PROTECT", form.span),
                result_form,
                close_form,
            ],
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

    fn expand_with_input_from_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-input-from-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-input-from-string binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-input-from-string binding needs a stream variable and string",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-input-from-string stream variable must be a symbol",
        )?;

        let options = &binding[2..];
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "with-input-from-string options need keyword/value pairs",
                binding_form.span,
            ));
        }
        let mut start = None;
        let mut end = None;
        let mut index = None;
        for pair in options.chunks_exact(2) {
            let Some(keyword) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "with-input-from-string option must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword.as_str() {
                "START" => {
                    if start.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :start may appear only once",
                            pair[0].span,
                        ));
                    }
                    start = Some(pair[1].clone());
                }
                "END" => {
                    if end.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :end may appear only once",
                            pair[0].span,
                        ));
                    }
                    end = Some(pair[1].clone());
                }
                "INDEX" => {
                    if index.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :index may appear only once",
                            pair[0].span,
                        ));
                    }
                    index = Some(pair[1].clone());
                }
                _ => {
                    return Err(self.invalid(
                        "with-input-from-string option is not supported",
                        pair[0].span,
                    ));
                }
            }
        }

        let mut input_items = Vec::with_capacity(4);
        input_items.push(Form::atom("MAKE-STRING-INPUT-STREAM", binding_form.span));
        input_items.push(binding[1].clone());
        match (start, end) {
            (None, None) => {}
            (Some(start), None) => input_items.push(start),
            (None, Some(end)) => {
                input_items.push(Form::atom("0", binding_form.span));
                input_items.push(end);
            }
            (Some(start), Some(end)) => {
                input_items.push(start);
                input_items.push(end);
            }
        }
        let input_form = Form::list(input_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), input_form],
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
        let body = if let Some(index) = index {
            let stream_position_form = Form::list(
                vec![
                    Form::atom("%STREAM-INPUT-POSITION", form.span),
                    binding[0].clone(),
                ],
                form.span,
            );
            let setf_form = Form::list(
                vec![Form::atom("SETF", form.span), index, stream_position_form],
                form.span,
            );
            Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    body,
                    setf_form,
                ],
                form.span,
            )
        } else {
            body
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

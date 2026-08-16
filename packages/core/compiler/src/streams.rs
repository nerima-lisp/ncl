use super::*;

impl CompileState {
    pub(super) fn compile_with_open_file(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OPEN-FILE", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(span, "missing WITH-OPEN-FILE binding after arity check")
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OPEN-FILE binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-OPEN-FILE binding needs a stream variable and pathname"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OPEN-FILE stream variable")?;

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
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    pub(super) fn compile_with_output_to_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-OUTPUT-TO-STRING", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing WITH-OUTPUT-TO-STRING binding after arity check",
            )
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-OUTPUT-TO-STRING binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if !(1..=2).contains(&binding.len()) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message:
                        "WITH-OUTPUT-TO-STRING binding needs a stream variable and optional string place"
                            .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-OUTPUT-TO-STRING stream variable")?;

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
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let output_string_form = Form::list(
            vec![
                Form::atom("GET-OUTPUT-STREAM-STRING", span),
                binding[0].clone(),
            ],
            span,
        );
        let result_form = if let Some(string_place) = binding.get(1) {
            let append_form = Form::list(
                vec![
                    Form::atom("__NCL_APPEND_OUTPUT_TO_STRING", span),
                    string_place.clone(),
                    output_string_form,
                ],
                span,
            );
            let setf_form = Form::list(
                vec![Form::atom("SETF", span), string_place.clone(), append_form],
                span,
            );
            Form::list(
                vec![Form::atom("MULTIPLE-VALUE-PROG1", span), body, setf_form],
                span,
            )
        } else {
            Form::list(
                vec![Form::atom("PROGN", span), body, output_string_form],
                span,
            )
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), result_form, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }

    pub(super) fn compile_with_input_from_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-INPUT-FROM-STRING", "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing WITH-INPUT-FROM-STRING binding after arity check",
            )
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-INPUT-FROM-STRING binding".to_string(),
                },
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-INPUT-FROM-STRING binding needs a stream variable and string"
                        .to_string(),
                },
                binding_form.span,
            ));
        }
        self.symbol_name(&binding[0], "WITH-INPUT-FROM-STRING stream variable")?;

        let options = &binding[2..];
        if options.len() % 2 != 0 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-INPUT-FROM-STRING options need keyword/value pairs".to_string(),
                },
                binding_form.span,
            ));
        }
        let mut start = None;
        let mut end = None;
        let mut index = None;
        for pair in options.chunks_exact(2) {
            let keyword = match &pair[0].kind {
                FormKind::Atom(name) if name.starts_with(':') && name.len() > 1 => {
                    normalize_name(&name[1..])
                }
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "WITH-INPUT-FROM-STRING option must be a keyword".to_string(),
                        },
                        pair[0].span,
                    ));
                }
            };
            match keyword.as_str() {
                "START" => {
                    if start.is_some() {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "WITH-INPUT-FROM-STRING :start may appear only once"
                                    .to_string(),
                            },
                            pair[0].span,
                        ));
                    }
                    start = Some(pair[1].clone());
                }
                "END" => {
                    if end.is_some() {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "WITH-INPUT-FROM-STRING :end may appear only once"
                                    .to_string(),
                            },
                            pair[0].span,
                        ));
                    }
                    end = Some(pair[1].clone());
                }
                "INDEX" => {
                    if index.is_some() {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "WITH-INPUT-FROM-STRING :index may appear only once"
                                    .to_string(),
                            },
                            pair[0].span,
                        ));
                    }
                    index = Some(pair[1].clone());
                }
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "WITH-INPUT-FROM-STRING option is not supported".to_string(),
                        },
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
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let body = if let Some(index) = index {
            let stream_position_form = Form::list(
                vec![
                    Form::atom("%STREAM-INPUT-POSITION", span),
                    binding[0].clone(),
                ],
                span,
            );
            let setf_form = Form::list(
                vec![Form::atom("SETF", span), index, stream_position_form],
                span,
            );
            Form::list(
                vec![Form::atom("MULTIPLE-VALUE-PROG1", span), body, setf_form],
                span,
            )
        } else {
            body
        };
        let close_form = Form::list(vec![Form::atom("CLOSE", span), binding[0].clone()], span);
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", span), body, close_form],
            span,
        );
        let expanded = Form::list(
            vec![Form::atom("LET", span), generated_binding, protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }
}

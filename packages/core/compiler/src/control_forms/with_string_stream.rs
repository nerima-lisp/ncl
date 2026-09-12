#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_with_input_from_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_with_string_stream(function, span, items, true, "WITH-INPUT-FROM-STRING")
    }

    pub(crate) fn compile_with_output_to_string(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_with_string_stream(function, span, items, false, "WITH-OUTPUT-TO-STRING")
    }

    fn compile_with_string_stream(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        input: bool,
        operator: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, operator, "at least one", span));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: format!("{operator} binding"),
                },
                items[1].span,
            ));
        };
        if binding.is_empty() || (input && binding.len() < 2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: format!(
                        "{operator} binding needs a variable{}",
                        if input { " and string form" } else { "" }
                    ),
                },
                items[1].span,
            ));
        }
        let variable = Self::symbol_name(&binding[0], &format!("{operator} variable"))?;
        let destination = if !input && binding.len() > 1 {
            Some(Self::symbol_name(
                &binding[1],
                &format!("{operator} destination"),
            )?)
        } else {
            None
        };
        let stream = self.reserve_function(None, Vec::new());
        let mut stream_form = vec![Form::atom(
            if input {
                "MAKE-STRING-INPUT-STREAM"
            } else {
                "MAKE-STRING-OUTPUT-STREAM"
            },
            span,
        )];
        let mut index = None;
        if input {
            let mut start = None;
            let mut end = None;
            let mut options = binding[1..].iter();
            while let Some(option) = options.next() {
                let FormKind::Atom(name) = &option.kind else {
                    stream_form.push(option.clone());
                    continue;
                };
                match name.to_ascii_uppercase().as_str() {
                    ":INDEX" => {
                        let target = options.next().ok_or_else(|| {
                            CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: format!("{operator} :INDEX needs a variable"),
                                },
                                option.span,
                            )
                        })?;
                        index = Some(Self::symbol_name(
                            target,
                            &format!("{operator} index variable"),
                        )?);
                    }
                    ":START" => {
                        start = Some(
                            options
                                .next()
                                .ok_or_else(|| {
                                    CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: format!("{operator} :START needs a value"),
                                        },
                                        option.span,
                                    )
                                })?
                                .clone(),
                        );
                    }
                    ":END" => {
                        end = Some(
                            options
                                .next()
                                .ok_or_else(|| {
                                    CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: format!("{operator} :END needs a value"),
                                        },
                                        option.span,
                                    )
                                })?
                                .clone(),
                        );
                    }
                    _ => stream_form.push(option.clone()),
                }
            }
            if let Some(start) = start {
                stream_form.push(start);
                if let Some(end) = end {
                    stream_form.push(end);
                }
            } else if let Some(end) = end {
                stream_form.push(Form::atom("0", span));
                stream_form.push(end);
            }
        }
        self.compile_expression(stream, &Form::list(stream_form, span))?;
        self.emit(stream, Instruction::Return, span)?;
        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, &items[2..])?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::StandardStreamBind {
                input,
                stream,
                variable,
                index,
                destination,
                body,
            },
            span,
        )?;
        Ok(())
    }
}

#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_with_open_file(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_with_open(function, span, items, true, "WITH-OPEN-FILE")
    }

    pub(crate) fn compile_with_open_stream(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_with_open(function, span, items, false, "WITH-OPEN-STREAM")
    }

    fn compile_with_open(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        open_pathnames: bool,
        operator: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, operator, "at least one", span));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            Self::internal_error(span, &format!("missing {operator} binding after arity check"))
        })?;
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: format!("{operator} binding"),
                },
                binding_form.span,
            ));
        };
        if binding.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: format!("{operator} needs at least one binding"),
                },
                binding_form.span,
            ));
        }
        let mut generated_bindings = Vec::with_capacity(binding.len());
        let mut stream_names = Vec::with_capacity(binding.len());
        for binding in binding {
            let FormKind::List(binding) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: format!("{operator} binding"),
                    },
                    binding.span,
                ));
            };
            if binding.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: format!("{operator} binding needs a stream variable and stream form"),
                    },
                    binding_form.span,
                ));
            }
            Self::symbol_name(&binding[0], &format!("{operator} stream variable"))?;
            let value = if open_pathnames {
                let mut open_items = Vec::with_capacity(binding.len());
                open_items.push(Form::atom("OPEN", binding_form.span));
                open_items.extend(binding[1..].iter().cloned());
                Form::list(open_items, binding_form.span)
            } else {
                binding[1].clone()
            };
            generated_bindings.push(Form::list(
                vec![binding[0].clone(), value],
                binding_form.span,
            ));
            stream_names.push(binding[0].clone());
        }
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, span)
        } else {
            Form::atom("NIL", span)
        };
        let protected_form = stream_names.into_iter().rev().fold(body, |body, stream| {
            Form::list(
                vec![
                    Form::atom("UNWIND-PROTECT", span),
                    body,
                    Form::list(vec![Form::atom("CLOSE", span), stream], span),
                ],
                span,
            )
        });
        let expanded = Form::list(
            vec![Form::atom("LET", span), Form::list(generated_bindings, binding_form.span), protected_form],
            span,
        );
        self.compile_expression(function, &expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_with_open_file_expands_to_nil_when_the_body_is_empty() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let binding = Form::list(
            vec![
                Form::list(
                    vec![
                        Form::atom("S", span),
                        Form::new(FormKind::String("f.txt".to_string()), span),
                    ],
                    span,
                ),
            ],
            span,
        );
        let items = vec![Form::atom("WITH-OPEN-FILE", span), binding];

        state
            .compile_with_open_file(function, span, &items)
            .unwrap_or_else(|error| {
                panic!("an empty body expands into a NIL-returning UNWIND-PROTECT: {error}")
            });

        assert!(
            !state.functions[function].instructions.is_empty(),
            "expanding WITH-OPEN-FILE should emit bytecode"
        );
    }

    #[test]
    fn compile_with_open_file_accepts_multiple_bindings() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let binding = Form::list(
            vec![
                Form::list(
                    vec![Form::atom("S", span), Form::new(FormKind::String("a.txt".to_string()), span)],
                    span,
                ),
                Form::list(
                    vec![Form::atom("U", span), Form::new(FormKind::String("b.txt".to_string()), span)],
                    span,
                ),
            ],
            span,
        );
        let items = vec![Form::atom("WITH-OPEN-FILE", span), binding];

        state.compile_with_open_file(function, span, &items).unwrap();
        assert!(state.functions[function].instructions.len() >= 2);
    }
}

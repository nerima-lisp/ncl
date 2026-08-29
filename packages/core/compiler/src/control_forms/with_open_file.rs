#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_with_open_file(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "WITH-OPEN-FILE",
                "at least one",
                span,
            ));
        }
        let binding_form = items.get(1).ok_or_else(|| {
            Self::internal_error(span, "missing WITH-OPEN-FILE binding after arity check")
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
        Self::symbol_name(&binding[0], "WITH-OPEN-FILE stream variable")?;

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
}

#![allow(clippy::wildcard_imports)]
use crate::*;

mod auxiliary;
mod default;
mod keyword;
mod lambda_list;
mod lambda_list_markers;
mod optional;
mod parameter_section;
mod pattern;

impl CompileState {
    pub(crate) fn compile_destructuring_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "DESTRUCTURING-BIND",
                "two or more",
                span,
            ));
        }
        let mut seen = HashSet::new();
        let specification = match &items[1].kind {
            FormKind::List(_) => {
                DestructureSpec::LambdaList(self.compile_destructuring_lambda_list(&items[1])?)
            }
            _ => {
                DestructureSpec::Pattern(Self::compile_destructuring_pattern(&items[1], &mut seen)?)
            }
        };
        self.emit(function, Instruction::EnterScope, items[1].span)?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::Destructure(specification),
            items[1].span,
        )?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_destructuring_bind_rejects_too_few_items() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("DESTRUCTURING-BIND", span),
            Form::list(Vec::new(), span),
        ];

        let error = state
            .compile_destructuring_bind(function, span, &items)
            .map_or_else(
                |error| error,
                |value| {
                    panic!(
                        "DESTRUCTURING-BIND needs a lambda list and an expression, got {value:?}"
                    )
                },
            );

        assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
    }

    #[test]
    fn compile_destructuring_bind_propagates_an_expression_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("DESTRUCTURING-BIND", span),
            Form::list(vec![Form::atom("A", span)], span),
            Form::list(vec![Form::atom("DEFUN", span)], span),
        ];

        let error = state
            .compile_destructuring_bind(function, span, &items)
            .map_or_else(
                |error| error,
                |value| {
                    panic!("a malformed source expression should fail to compile, got {value:?}")
                },
            );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_bind_propagates_a_body_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let items = vec![
            Form::atom("DESTRUCTURING-BIND", span),
            Form::list(vec![Form::atom("A", span)], span),
            Form::atom("1", span),
            dotted,
        ];

        let error = state
            .compile_destructuring_bind(function, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed body form should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_destructuring_bind_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("DESTRUCTURING-BIND", span),
            Form::list(vec![Form::atom("A", span)], span),
            Form::atom("1", span),
        ];

        let error = state
            .compile_destructuring_bind(99, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }
}

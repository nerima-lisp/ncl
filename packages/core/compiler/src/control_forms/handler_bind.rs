#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_handler_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "HANDLER-BIND",
                "at least one",
                span,
            ));
        }
        let handler_form = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing HANDLER-BIND handler list"))?;
        let FormKind::List(handler_items) = &handler_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "handler-bind handler list".to_string(),
                },
                handler_form.span,
            ));
        };

        let mut handlers = Vec::with_capacity(handler_items.len());
        for handler in handler_items {
            let FormKind::List(handler_clause) = &handler.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-bind clause".to_string(),
                    },
                    handler.span,
                ));
            };
            if handler_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-bind clause needs a condition and handler".to_string(),
                    },
                    handler.span,
                ));
            }
            let condition = Self::condition_name(&handler_clause[0], "handler-bind condition")?;
            let condition_variable = self.fresh_name("HANDLER_CONDITION");
            let clause_function = self.reserve_function(None, vec![condition_variable.clone()]);
            self.compile_expression(clause_function, &handler_clause[1])?;
            self.compile_expression(
                clause_function,
                &Form::atom(condition_variable, handler_clause[1].span),
            )?;
            self.emit(
                clause_function,
                Instruction::Call(1),
                handler_clause[1].span,
            )?;
            self.emit(clause_function, Instruction::Return, handler.span)?;
            handlers.push(HandlerBindClause {
                condition,
                function: clause_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::HandlerBind {
                body: body_function,
                handlers,
            },
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dotted(span: Span) -> Form {
        Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
    }

    #[test]
    fn compile_handler_bind_propagates_a_handler_body_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let clause = Form::list(vec![Form::atom("MY-CONDITION", span), dotted(span)], span);
        let items = vec![
            Form::atom("HANDLER-BIND", span),
            Form::list(vec![clause], span),
        ];

        let error = state
            .compile_handler_bind(function, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed handler body should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_handler_bind_propagates_a_protected_body_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let clause = Form::list(
            vec![Form::atom("MY-CONDITION", span), Form::atom("F", span)],
            span,
        );
        let items = vec![
            Form::atom("HANDLER-BIND", span),
            Form::list(vec![clause], span),
            dotted(span),
        ];

        let error = state
            .compile_handler_bind(function, span, &items)
            .map_or_else(
                |error| error,
                |value| {
                    panic!("a malformed protected body form should fail to compile, got {value:?}")
                },
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_handler_bind_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let clause = Form::list(
            vec![Form::atom("MY-CONDITION", span), Form::atom("F", span)],
            span,
        );
        let items = vec![
            Form::atom("HANDLER-BIND", span),
            Form::list(vec![clause], span),
        ];

        let error = state.compile_handler_bind(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }
}

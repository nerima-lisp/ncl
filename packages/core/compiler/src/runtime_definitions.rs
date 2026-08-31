#[allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(super) fn compile_load_time_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(
                items,
                "LOAD-TIME-VALUE",
                "one or two",
                span,
            ));
        }
        self.compile_runtime_definition(function, span, items)
    }

    pub(super) fn compile_defstruct(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DEFSTRUCT", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    pub(super) fn compile_runtime_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "runtime definition",
                "at least one",
                span,
            ));
        }
        if let Some(result) = self.compile_native_push_pop(function, span, items)? {
            return Ok(result);
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    fn compile_native_push_pop(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        let Some(operator) = items
            .first()
            .and_then(|form| Self::symbol_name_info(form, "runtime operator").ok())
            .map(|(name, _)| name)
        else {
            return Ok(None);
        };
        if !matches!(operator.as_str(), "PUSH" | "POP") {
            return Ok(None);
        }
        let expected = if operator == "PUSH" { 3 } else { 2 };
        if items.len() != expected {
            return Err(Self::arity_error(
                items,
                &operator,
                if operator == "PUSH" { "two" } else { "one" },
                span,
            ));
        }
        let Some((name, escaped)) = Self::symbol_name_info(&items[expected - 1], "list place").ok()
        else {
            return Ok(None);
        };
        if operator == "PUSH" {
            self.compile_expression(function, &items[1])?;
        }
        self.compile_expression(function, &items[expected - 1])?;
        self.emit(
            function,
            if operator == "PUSH" {
                Instruction::PushList { name, escaped }
            } else {
                Instruction::PopList { name, escaped }
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_defstruct_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("DEFSTRUCT", span), Form::atom("POINT", span)];

        let error = state.compile_defstruct(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_runtime_definition_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("DEFPACKAGE", span), Form::atom("FOO", span)];

        let error = state
            .compile_runtime_definition(99, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_load_time_value_rejects_more_than_two_arguments() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("LOAD-TIME-VALUE", span),
            Form::atom("1", span),
            Form::atom("NIL", span),
            Form::atom("NIL", span),
        ];

        let Err(error) = state.compile_load_time_value(function, span, &items) else {
            panic!("too many LOAD-TIME-VALUE arguments must fail during compilation")
        };

        assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
    }
}

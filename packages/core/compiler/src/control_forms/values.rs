#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_nth_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "NTH-VALUE", "two", 2, span)?;
        let [_, index, producer] = items else {
            return Err(Self::internal_error(
                span,
                "missing NTH-VALUE operands after arity check",
            ));
        };
        self.compile_expression(function, index)?;
        self.emit(function, Instruction::Primary, index.span)?;
        self.emit(
            function,
            Instruction::CheckNthValueIndex(index.span),
            index.span,
        )?;
        self.compile_expression(function, producer)?;
        self.emit(function, Instruction::NthValue, index.span)?;
        Ok(())
    }

    pub(crate) fn compile_values(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        for item in items.get(1..).unwrap_or(&[]) {
            self.compile_expression(function, item)?;
            self.emit(function, Instruction::Primary, item.span)?;
        }
        self.emit(
            function,
            Instruction::Values(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_multiple_value_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "MULTIPLE-VALUE-LIST", "one", 1, span)?;
        let Some(value_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-LIST value form after arity check",
            ));
        };
        self.compile_expression(function, value_form)?;
        self.emit(function, Instruction::MultipleValueList, value_form.span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_value_compiles_validation_before_the_producer_without_eval() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let form = Form::list(
            vec![
                Form::atom("NTH-VALUE", span),
                Form::atom("0", span),
                Form::list(vec![Form::atom("VALUES", span)], span),
            ],
            span,
        );
        state
            .compile_expression(function, &form)
            .unwrap_or_else(|error| panic!("NTH-VALUE should compile: {error}"));
        assert_eq!(
            state.functions[function].instructions,
            vec![
                Instruction::Constant(Constant::Integer(0)),
                Instruction::Primary,
                Instruction::CheckNthValueIndex(span),
                Instruction::Values(0),
                Instruction::NthValue,
            ]
        );
    }

    #[test]
    fn nth_value_rejects_wrong_arity_without_emitting_code() {
        for count in [0, 1, 3] {
            let mut state = CompileState::default();
            let function = state.reserve_function(None, Vec::new());
            let span = Span::new(0, 1);
            let mut items = vec![Form::atom("NTH-VALUE", span)];
            items.extend((0..count).map(|_| Form::atom("0", span)));
            assert!(state.compile_nth_value(function, span, &items).is_err());
            assert!(state.functions[function].instructions.is_empty());
        }
    }

    #[test]
    fn compile_values_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let items = vec![Form::atom("VALUES", span), dotted];

        let error = state.compile_values(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_values_reports_an_internal_error_for_an_invalid_function_id_with_no_arguments() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("VALUES", span)];

        let error = state.compile_values(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_multiple_value_list_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let items = vec![Form::atom("MULTIPLE-VALUE-LIST", span), dotted];

        let error = state
            .compile_multiple_value_list(function, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed argument should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }
}

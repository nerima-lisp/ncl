use crate::{CompileError, CompileState, Form, FunctionId, Instruction, Span};

impl CompileState {
    pub(super) fn compile_quote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "QUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing quote argument after arity check",
            ));
        };
        self.emit(
            function,
            Instruction::Quote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }

    pub(super) fn compile_quasiquote(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "QUASIQUOTE", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing quasiquote argument after arity check",
            ));
        };
        self.emit(
            function,
            Instruction::QuasiQuote(argument.clone()),
            argument.span,
        )?;
        Ok(())
    }
}

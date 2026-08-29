#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
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

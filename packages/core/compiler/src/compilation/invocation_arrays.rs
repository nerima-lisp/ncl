use super::*;

impl CompileState {
    pub(crate) fn compile_vector(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::VectorConstruction {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_array_construction(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "MAKE-ARRAY", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ArrayConstruction {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

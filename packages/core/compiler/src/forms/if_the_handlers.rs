use crate::{CompileError, CompileState, Constant, Form, FunctionId, Instruction, Span};

impl CompileState {
    pub(super) fn compile_if(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(Self::arity_error(items, "IF", "two or three", span));
        }
        let Some(condition) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing if condition after arity check",
            ));
        };
        let Some(then_branch) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing if branch after arity check",
            ));
        };

        self.compile_expression(function, condition)?;
        let false_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            condition.span,
        )?;
        self.compile_expression(function, then_branch)?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), then_branch.span)?;
        let else_target = self.instruction_count(function, span)?;
        self.patch_jump(function, false_jump, else_target, condition.span)?;

        if let Some(else_branch) = items.get(3) {
            self.compile_expression(function, else_branch)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
    }

    pub(super) fn compile_the(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "THE", "two", 2, span)?;
        let Some(type_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing THE type after arity check",
            ));
        };
        let Some(value_form) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing THE value after arity check",
            ));
        };
        self.emit(
            function,
            Instruction::FunctionLoad("__NCL_THE_CHECK".to_string()),
            span,
        )?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::Quote(type_form.clone()),
            type_form.span,
        )?;
        self.emit(function, Instruction::Call(2), span)?;
        Ok(())
    }
}

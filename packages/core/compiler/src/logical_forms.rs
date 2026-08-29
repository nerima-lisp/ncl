use super::{
    CompileError, CompileErrorKind, CompileState, Constant, Form, FunctionId, Instruction, Span,
    operator_span,
};

impl CompileState {
    pub(super) fn compile_and(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        let Some((last, prefix)) = forms.split_last() else {
            self.emit(
                function,
                Instruction::Constant(Constant::Boolean(true)),
                span,
            )?;
            return Ok(());
        };

        let mut false_jumps = Vec::with_capacity(prefix.len());
        for form in prefix {
            self.compile_expression(function, form)?;
            self.emit(function, Instruction::Dup, form.span)?;
            let jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), form.span)?;
            false_jumps.push(jump);
            self.emit(function, Instruction::Pop, form.span)?;
        }
        self.compile_expression(function, last)?;

        let end = self.instruction_count(function, span)?;
        for jump in false_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }

    pub(super) fn compile_or(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let forms = items.get(1..).unwrap_or(&[]);
        let Some((last, prefix)) = forms.split_last() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for form in prefix {
            self.compile_expression(function, form)?;
            self.emit(function, Instruction::Dup, form.span)?;
            let false_jump =
                self.emit(function, Instruction::JumpIfFalse(usize::MAX), form.span)?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), form.span)?;
            let next = self.instruction_count(function, span)?;
            self.patch_jump(function, false_jump, next, form.span)?;
            self.emit(function, Instruction::Pop, form.span)?;
            end_jumps.push(end_jump);
        }
        self.compile_expression(function, last)?;

        let end = self.instruction_count(function, span)?;
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }

    pub(super) fn compile_when(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        positive: bool,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: if positive { "WHEN" } else { "UNLESS" }.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let condition = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing when condition"))?;
        self.compile_expression(function, condition)?;
        let branch_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            condition.span,
        )?;

        if positive {
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
            let false_target = self.instruction_count(function, span)?;
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            let end_target = self.instruction_count(function, span)?;
            self.patch_jump(function, branch_jump, false_target, condition.span)?;
            self.patch_jump(function, end_jump, end_target, span)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
            let body_target = self.instruction_count(function, span)?;
            self.compile_sequence(function, items.get(2..).unwrap_or(&[]))?;
            let end_target = self.instruction_count(function, span)?;
            self.patch_jump(function, branch_jump, body_target, condition.span)?;
            self.patch_jump(function, end_jump, end_target, span)?;
        }
        Ok(())
    }
}

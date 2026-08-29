use crate::{
    CompileError, CompileErrorKind, CompileState, Constant, Form, FormKind, FunctionId,
    Instruction, Span,
};

impl CompileState {
    pub(crate) fn compile_cond(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let clauses = items.get(1..).unwrap_or(&[]);
        let mut end_jumps = Vec::with_capacity(clauses.len());
        for clause in clauses {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "cond clause".to_string(),
                    },
                    clause.span,
                ));
            };
            let Some(condition) = clause_items.first() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "cond clause cannot be empty".to_string(),
                    },
                    clause.span,
                ));
            };
            self.compile_expression(function, condition)?;
            if clause_items.len() == 1 {
                self.emit(function, Instruction::Dup, condition.span)?;
                let false_jump = self.emit(
                    function,
                    Instruction::JumpIfFalse(usize::MAX),
                    condition.span,
                )?;
                let end_jump = self.emit(function, Instruction::Jump(usize::MAX), clause.span)?;
                let next_clause = self.instruction_count(function, clause.span)?;
                self.patch_jump(function, false_jump, next_clause, condition.span)?;
                self.emit(function, Instruction::Pop, condition.span)?;
                end_jumps.push(end_jump);
            } else {
                let false_jump = self.emit(
                    function,
                    Instruction::JumpIfFalse(usize::MAX),
                    condition.span,
                )?;
                self.compile_sequence(function, &clause_items[1..])?;
                let end_jump = self.emit(function, Instruction::Jump(usize::MAX), clause.span)?;
                let next_clause = self.instruction_count(function, clause.span)?;
                self.patch_jump(function, false_jump, next_clause, condition.span)?;
                end_jumps.push(end_jump);
            }
        }
        self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        let end = self.instruction_count(function, span)?;
        for jump in end_jumps {
            self.patch_jump(function, jump, end, span)?;
        }
        Ok(())
    }
}

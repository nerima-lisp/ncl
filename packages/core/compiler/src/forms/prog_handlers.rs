use crate::{CompileError, CompileState, Form, FunctionId, Instruction, Span};

impl CompileState {
    pub(super) fn compile_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "PROG1", "at least one", span));
        }

        let Some(first) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing PROG1 form after arity check",
            ));
        };
        let retained = self.fresh_name("PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Define(retained.clone()), first.span)?;
        self.emit(function, Instruction::Pop, first.span)?;

        let tail = items.get(2..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_prog2(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "PROG2", "at least two", span));
        }

        let Some(first) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing first PROG2 form after arity check",
            ));
        };
        let Some(second) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing second PROG2 form after arity check",
            ));
        };
        let retained = self.fresh_name("PROG2_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(function, Instruction::Pop, first.span)?;
        self.compile_expression(function, second)?;
        self.emit(function, Instruction::Define(retained.clone()), second.span)?;
        self.emit(function, Instruction::Pop, second.span)?;

        let tail = items.get(3..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

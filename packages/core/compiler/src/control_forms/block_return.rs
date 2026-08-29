#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_block(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "BLOCK", "at least one", span));
        }
        let name = Self::control_name(
            items.get(1).ok_or_else(|| {
                Self::internal_error(span, "missing BLOCK name after arity check")
            })?,
            "BLOCK name",
        )?;
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, items.get(2..).unwrap_or(&[]))?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: child,
                name,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_return(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 1 || items.len() == 2) {
            return Err(Self::arity_error(items, "RETURN", "zero or one", span));
        }
        if let Some(value) = items.get(1) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            Instruction::ReturnFrom {
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_return_from(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity_error(items, "RETURN-FROM", "one or two", span));
        }
        let name = Self::control_name(
            items.get(1).ok_or_else(|| {
                Self::internal_error(span, "missing RETURN-FROM name after arity check")
            })?,
            "RETURN-FROM name",
        )?;
        if let Some(value) = items.get(2) {
            self.compile_expression(function, value)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ReturnFrom { name }, span)?;
        Ok(())
    }
}

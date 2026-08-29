#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_catch(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "CATCH", "at least one", span));
        }

        let tag_function = self.reserve_function(None, Vec::new());
        self.compile_expression(tag_function, &items[1])?;
        self.emit(tag_function, Instruction::Return, items[1].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Catch {
                tag: tag_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_throw(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(items, "THROW", "two", span));
        }

        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Throw, span)?;
        Ok(())
    }

    pub(crate) fn compile_progv(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "PROGV", "at least two", span));
        }

        let symbols_function = self.reserve_function(None, Vec::new());
        self.compile_expression(symbols_function, &items[1])?;
        self.emit(symbols_function, Instruction::Return, items[1].span)?;

        let values_function = self.reserve_function(None, Vec::new());
        self.compile_expression(values_function, &items[2])?;
        self.emit(values_function, Instruction::Return, items[2].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(3..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;

        self.emit(
            function,
            Instruction::Progv {
                symbols: symbols_function,
                values: values_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_unwind_protect(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "UNWIND-PROTECT",
                "at least one",
                span,
            ));
        }

        let protected = items.get(1).ok_or_else(|| {
            Self::internal_error(
                span,
                "missing UNWIND-PROTECT protected form after arity check",
            )
        })?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let cleanup_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(cleanup_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(cleanup_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::UnwindProtect {
                protected: protected_function,
                cleanup: cleanup_function,
            },
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

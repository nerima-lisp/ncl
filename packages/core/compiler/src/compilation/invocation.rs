#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_funcall(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "FUNCALL", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_eval(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "EVAL", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing eval argument after arity check",
            ));
        };
        self.compile_expression(function, argument)?;
        self.emit(function, Instruction::Eval(argument.span), span)?;
        Ok(())
    }

    pub(crate) fn compile_apply(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "APPLY", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Apply(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_mapcar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAPCAR", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MapCar(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_map_into(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAP-INTO", "at least two", span));
        }
        let destination = items[1].clone();
        self.emit(
            function,
            Instruction::FunctionLoad("MAP-INTO".to_string()),
            items[0].span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        self.emit(
            function,
            Instruction::MapIntoSetf(destination.clone()),
            destination.span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

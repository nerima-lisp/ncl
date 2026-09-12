#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(crate) fn compile_type_predicate(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() != 2 {
            return Err(Self::arity_error(items, operation, "one", span));
        }
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::TypePredicate {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_typep(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(items, "TYPEP", "two", span));
        }
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Typep, span)?;
        Ok(())
    }

    pub(crate) fn compile_symbol_unary(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() != 2 {
            return Err(Self::arity_error(items, operation, "one", span));
        }
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::SymbolUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_value_unary(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() != 2 {
            return Err(Self::arity_error(items, operation, "one", span));
        }
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::ValueUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }
}

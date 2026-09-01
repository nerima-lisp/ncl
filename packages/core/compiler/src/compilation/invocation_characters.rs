#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_character_unary(
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
            Instruction::CharacterUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_character_predicate(
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
            Instruction::CharacterPredicate {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_character_digit_predicate(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(items, "DIGIT-CHAR-P", "one or two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::CharacterDigitPredicate {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

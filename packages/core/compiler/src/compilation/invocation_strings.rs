#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_string_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if !(2..=6).contains(&items.len()) || !(items.len() - 2).is_multiple_of(2) {
            return Err(Self::arity_error(items, operation, "1, 3, or 5", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::StringCase {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_string_comparison(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "two", 2, span)?;
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::StringComparison {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_string_trim(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "two", 2, span)?;
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::StringTrim {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_string_construction(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let argument_count = match operation {
            "STRING" => {
                Self::require_arity(items, operation, "one", 1, span)?;
                1
            }
            "MAKE-STRING" => {
                if !(2..=4).contains(&items.len()) {
                    return Err(Self::arity_error(items, operation, "one or two arguments or keyword/value pairs", span));
                }
                items.len() - 1
            }
            _ => return Err(Self::arity_error(items, operation, "valid", span)),
        };
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::StringConstruction {
                operation: operation.to_string(),
                argument_count,
            },
            span,
        )?;
        Ok(())
    }
}

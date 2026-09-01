#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_package_introspection(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "one", 1, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(
            function,
            Instruction::PackageIntrospection {
                operation: operation.to_string(),
                argument_count: 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_package_mutation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(items, operation, "one or two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::PackageMutation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_package_listing(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "DOCUMENTATION" => (items.len() == 3, "two"),
            "LIST-ALL-PACKAGES" => (items.len() == 1, "zero"),
            _ => (false, "valid arguments"),
        };
        if !valid_arity {
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::PackageListing {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

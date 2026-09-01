#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_list_tail(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(items, operation, "one or two", span));
        }
        self.compile_expression(function, &items[1])?;
        for item in &items[2..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ListTail {
                operation: operation.to_string(),
                option_count: items.len() - 2,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_binary(
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
            Instruction::ListBinary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_set(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, operation, "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ListSet {
                operation: operation.to_string(),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_tree_equal(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "TREE-EQUAL", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::TreeEqual {
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_construction_with_options(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "MAKE-LIST", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ListConstructionWithOptions {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_construction(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if operation == "LIST*" && items.len() < 2 {
            return Err(Self::arity_error(items, operation, "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ListConstruction {
                argument_count: items.len().saturating_sub(1),
                dotted: operation == "LIST*",
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_append(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = match operation {
            "ACONS" => items.len() == 4,
            "PAIRLIS" => (3..=4).contains(&items.len()),
            "REVAPPEND" | "NRECONC" => items.len() == 3,
            _ => items.len() >= 2,
        };
        if !valid_arity {
            let expected = match operation {
                "ACONS" => "three",
                "PAIRLIS" => "two or three",
                "REVAPPEND" | "NRECONC" => "two",
                _ => "at least one",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ListAppend {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

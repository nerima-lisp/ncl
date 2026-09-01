#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_list_membership(
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
            Instruction::ListMembership {
                operation: operation.to_string(),
                predicate: operation.ends_with("-IF") || operation.ends_with("-IF-NOT"),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_association_search(
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
            Instruction::AssociationSearch {
                operation: operation.to_string(),
                predicate: operation.ends_with("-IF") || operation.ends_with("-IF-NOT"),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_removal(
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
            Instruction::SequenceRemoval {
                operation: operation.to_string(),
                predicate: operation.ends_with("-IF") || operation.ends_with("-IF-NOT"),
                duplicates: operation.ends_with("DUPLICATES"),
                option_count: if operation.ends_with("DUPLICATES") {
                    items.len().saturating_sub(2)
                } else {
                    items.len().saturating_sub(3)
                },
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_substitution(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(items, operation, "at least three", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceSubstitution {
                operation: operation.to_string(),
                predicate: operation.ends_with("-IF") || operation.ends_with("-IF-NOT"),
                option_count: items.len().saturating_sub(4),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_unary(
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
            Instruction::SequenceUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_list_unary(
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
            Instruction::ListUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }
}

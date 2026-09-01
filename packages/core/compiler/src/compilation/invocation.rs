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

    pub(crate) fn compile_list_mapping(
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
            Instruction::ListMapping {
                operation: operation.to_string(),
                sequence_count: items.len().saturating_sub(2),
            },
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
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceMapInto {
                sequence_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        let destination = items[1].clone();
        self.emit(
            function,
            match Self::symbol_name_info(&destination, "MAP-INTO destination") {
                Ok((name, escaped)) => Instruction::MapIntoSetfSymbol { name, escaped },
                Err(_) => Instruction::MapIntoSetf(destination.clone()),
            },
            destination.span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_quantifier(
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
            Instruction::SequenceQuantifier {
                operation: operation.to_string(),
                sequence_count: items.len().saturating_sub(2),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_mapping(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(items, "MAP", "at least three", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceMapping {
                sequence_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_reduce(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "REDUCE", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceReduce {
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_merge(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 5 {
            return Err(Self::arity_error(items, "MERGE", "at least four", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceMerge {
                option_count: items.len().saturating_sub(5),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_sort(
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
            Instruction::SequenceSort {
                operation: operation.to_string(),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_search(
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
            Instruction::SequenceSearch {
                operation: operation.to_string(),
                predicate: operation.ends_with("-IF") || operation.ends_with("-IF-NOT"),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_pair_search(
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
            Instruction::SequencePairSearch {
                operation: operation.to_string(),
                option_count: items.len().saturating_sub(3),
            },
            span,
        )?;
        Ok(())
    }

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
}

#[cfg(test)]
mod tests;

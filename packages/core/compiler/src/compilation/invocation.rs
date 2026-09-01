#![allow(clippy::wildcard_imports)]
use super::*;

#[path = "invocation_calls.rs"]
mod invocation_calls;

#[path = "invocation_arrays.rs"]
mod invocation_arrays;

#[path = "invocation_sequences.rs"]
mod invocation_sequences;

#[path = "invocation_sequence_ops.rs"]
mod invocation_sequence_ops;

#[path = "invocation_sequence_collections.rs"]
mod invocation_sequence_collections;

#[path = "invocation_list.rs"]
mod invocation_list;

#[path = "invocation_hash.rs"]
mod invocation_hash;

#[path = "invocation_packages.rs"]
mod invocation_packages;

#[path = "invocation_symbols.rs"]
mod invocation_symbols;

#[path = "invocation_object_system.rs"]
mod invocation_object_system;

#[path = "invocation_atoms.rs"]
mod invocation_atoms;
#[path = "invocation_evaluation.rs"]
mod invocation_evaluation;
#[path = "invocation_numeric.rs"]
mod invocation_numeric;

#[path = "invocation_strings.rs"]
mod invocation_strings;

#[path = "invocation_characters.rs"]
mod invocation_characters;

#[path = "invocation_streams.rs"]
mod invocation_streams;

impl CompileState {
    pub(crate) fn compile_sequence_length(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "LENGTH", "one", 1, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(function, Instruction::SequenceLength, span)?;
        Ok(())
    }

    pub(crate) fn compile_sequence_element(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "ELT", "two", 2, span)?;
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::SequenceElement, span)?;
        Ok(())
    }

    pub(crate) fn compile_sequence_subseq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&(items.len() - 1)) {
            return Err(Self::arity_error(items, "SUBSEQ", "two or three", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceSubseq {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_mutation(
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
            Instruction::SequenceMutation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_concatenate(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "CONCATENATE",
                "a result type and at least one sequence",
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceConcatenate {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_sequence_conversion(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 3 || (operation == "COERCE" && items.len() != 3) {
            return Err(Self::arity_error(items, operation, "two or more", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SequenceConversion {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

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

#[path = "invocation_evaluation.rs"]
mod invocation_evaluation;
#[path = "invocation_atoms.rs"]
mod invocation_atoms;
#[path = "invocation_numeric.rs"]
mod invocation_numeric;

#[path = "invocation_strings.rs"]
mod invocation_strings;

#[path = "invocation_characters.rs"]
mod invocation_characters;

#[path = "invocation_streams.rs"]
mod invocation_streams;

impl CompileState {
    pub(crate) fn compile_numeric_float(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let argument_count = match operation {
            "FLOAT" | "FLOAT-SIGN" => 1..=2,
            "FLOAT-DIGITS"
            | "FLOAT-PRECISION"
            | "FLOAT-RADIX"
            | "DECODE-FLOAT"
            | "INTEGER-DECODE-FLOAT" => 1..=1,
            "LOG" | "ATAN" | "COMPLEX" => 1..=2,
            "SCALE-FLOAT" => 2..=2,
            _ => unreachable!("numeric float operation was not dispatched"),
        };
        if !argument_count.contains(&(items.len() - 1)) {
            let expected = match operation {
                "FLOAT" | "FLOAT-SIGN" => "one or two",
                "LOG" | "ATAN" | "COMPLEX" => "one or two",
                "SCALE-FLOAT" => "two",
                _ => "one",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::NumericFloat {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

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
        if items.len() != 3 {
            return Err(Self::arity_error(items, operation, "two", span));
        }
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

    pub(crate) fn compile_vector_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid = match operation {
            "FILL-POINTER" | "VECTOR-POP" => items.len() == 2,
            "VECTOR-PUSH" => items.len() == 3,
            "VECTOR-PUSH-EXTEND" => (3..=4).contains(&items.len()),
            _ => false,
        };
        if !valid {
            let expected = if operation == "VECTOR-PUSH-EXTEND" {
                "two or three"
            } else if operation == "VECTOR-PUSH" {
                "two"
            } else {
                "one"
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::VectorOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_integer_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 2 || !(items.len() - 2).is_multiple_of(2) {
            return Err(Self::arity_error(
                items,
                operation,
                "a string and keyword/value pairs",
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::IntegerOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
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

    pub(crate) fn compile_array_element(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
        exact_arity: bool,
    ) -> Result<(), CompileError> {
        if exact_arity {
            Self::require_arity(items, operation, "two", 2, span)?;
        } else if items.len() < 3 {
            return Err(Self::arity_error(items, operation, "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ArrayElement {
                operation: operation.to_string(),
                argument_count: items.len().saturating_sub(1),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_array_metadata(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
        argument_count: usize,
    ) -> Result<(), CompileError> {
        Self::require_arity(
            items,
            operation,
            &argument_count.to_string(),
            argument_count,
            span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ArrayMetadata {
                operation: operation.to_string(),
                argument_count,
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
}

#[cfg(test)]
mod tests;

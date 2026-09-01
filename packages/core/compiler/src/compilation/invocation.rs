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
            Instruction::ListUnary { operation: operation.to_string() },
            span,
        )?;
        Ok(())
    }

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
            Instruction::CharacterUnary { operation: operation.to_string() },
            span,
        )?;
        Ok(())
    }

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
        self.emit(function, Instruction::TypePredicate { operation: operation.to_string() }, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_unary(
        &mut self,
        function: usize,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "one", 1, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(function, Instruction::NumericUnary { operation: operation.to_string() }, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_comparison(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if matches!(operation, "REVAPPEND" | "NRECONC") {
            Self::require_arity(items, operation, "two", 2, span)?;
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::NumericComparison {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_fold(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::NumericFold {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_binary(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "two", 2, span)?;
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::NumericBinary { operation: operation.to_string() }, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_boole(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "BOOLE", "three", 3, span)?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::NumericBoole, span)?;
        Ok(())
    }

    pub(crate) fn compile_numeric_bitfield(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let argument_count = match operation {
            "BYTE" | "LDB" | "MASK-FIELD" => 2,
            "DPB" | "DEPOSIT-FIELD" => 3,
            _ => unreachable!("numeric bitfield operation was not dispatched"),
        };
        Self::require_arity(items, operation, &argument_count.to_string(), argument_count, span)?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::NumericBitfield {
                operation: operation.to_string(),
                argument_count,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_numeric_float(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let argument_count = match operation {
            "FLOAT-SIGN" => 1..=2,
            "FLOAT-DIGITS" | "FLOAT-PRECISION" | "FLOAT-RADIX" | "DECODE-FLOAT"
            | "INTEGER-DECODE-FLOAT" => 1..=1,
            "SCALE-FLOAT" => 2..=2,
            _ => unreachable!("numeric float operation was not dispatched"),
        };
        if !argument_count.contains(&(items.len() - 1)) {
            let expected = match operation {
                "FLOAT-SIGN" => "one or two",
                "SCALE-FLOAT" => "two",
                _ => "one",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::NumericFloat {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
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
        self.emit(function, Instruction::CharacterDigitPredicate { argument_count: items.len() - 1 }, span)?;
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
        self.emit(function, Instruction::ListBinary { operation: operation.to_string() }, span)?;
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
        self.emit(function, Instruction::SequenceSubseq { argument_count: items.len() - 1 }, span)?;
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
        self.emit(function, Instruction::SequenceMutation {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
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
            Instruction::StringTrim { operation: operation.to_string() },
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
            "STRING" => { Self::require_arity(items, operation, "one", 1, span)?; 1 }
            "MAKE-STRING" => {
                if !(2..=3).contains(&items.len()) {
                    return Err(Self::arity_error(items, operation, "one or two", span));
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
            Instruction::StringConstruction { operation: operation.to_string(), argument_count },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_vector(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::VectorConstruction {
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_array_construction(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "MAKE-ARRAY", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ArrayConstruction {
                argument_count: items.len() - 1,
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
            Instruction::ListConstructionWithOptions { argument_count: items.len() - 1 },
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
        self.emit(function, Instruction::ListAppend {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
        Ok(())
    }

    pub(crate) fn compile_property_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = match operation {
            "GETF" => (3..=4).contains(&items.len()),
            "GET-PROPERTIES" => items.len() == 3,
            "GET" => (3..=4).contains(&items.len()),
            "PUTPROP" => items.len() == 4,
            "REMPROP" => items.len() == 3,
            "SYMBOL-PLIST" => items.len() == 2,
            _ => false,
        };
        if !valid_arity {
            let expected = match operation {
                "GETF" | "GET" => "two or three",
                "PUTPROP" => "three",
                _ => "two",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::PropertyList {
            operation: operation.to_string(),
            argument_count: items.len() - 1,
        }, span)?;
        Ok(())
    }

    pub(crate) fn compile_symbol_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = match operation {
            "CONSTANTP" => (2..=3).contains(&items.len()),
            _ => items.len() == 2,
        };
        if !valid_arity {
            let expected = if operation == "CONSTANTP" { "one or two" } else { "one" };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(function, Instruction::SymbolValue { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_symbol_binding(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "SET" => (items.len() == 3, "two"),
            "MAKUNBOUND" | "FMAKUNBOUND" => (items.len() == 2, "one"),
            _ => (false, "valid arguments"),
        };
        if !valid_arity { return Err(Self::arity_error(items, operation, expected, span)); }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::SymbolBinding { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_symbol_function(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = match operation {
            "MACRO-FUNCTION" => (2..=3).contains(&items.len()),
            _ => items.len() == 2,
        };
        if !valid_arity {
            let expected = if operation == "MACRO-FUNCTION" { "one or two" } else { "one" };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::SymbolFunction { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_symbol_creation(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "MAKE-SYMBOL" => (items.len() == 2, "one"),
            "GENSYM" => ((1..=2).contains(&items.len()), "zero or one"),
            "INTERN" | "FIND-SYMBOL" => ((2..=3).contains(&items.len()), "one or two"),
            _ => (false, "valid arguments"),
        };
        if !valid_arity { return Err(Self::arity_error(items, operation, expected, span)); }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::SymbolCreation { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_package_introspection(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        Self::require_arity(items, operation, "one", 1, span)?;
        self.compile_expression(function, &items[1])?;
        self.emit(function, Instruction::PackageIntrospection { operation: operation.to_string(), argument_count: 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_package_mutation(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(items, operation, "one or two", span));
        }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::PackageMutation { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_package_listing(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "DOCUMENTATION" => (items.len() == 3, "two"),
            "LIST-ALL-PACKAGES" => (items.len() == 1, "zero"),
            _ => (false, "valid arguments"),
        };
        if !valid_arity { return Err(Self::arity_error(items, operation, expected, span)); }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::PackageListing { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_hash_table(
        &mut self, function: FunctionId, span: Span, items: &[Form], operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "GETHASH" => ((3..=4).contains(&items.len()), "two or three"),
            "REMHASH" => (items.len() == 3, "two"),
            "MAKE-HASH-TABLE" => ((items.len() - 1).is_multiple_of(2), "keyword/value pairs"),
            "CLRHASH" | "HASH-TABLE-COUNT" | "HASH-TABLE-TEST" | "NCL-HASH-TABLE-KEYS"
            | "NCL-HASH-TABLE-VALUES" => (items.len() == 2, "one"),
            _ => (false, "valid arguments"),
        };
        if !valid_arity {
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] { self.compile_expression(function, item)?; }
        self.emit(function, Instruction::HashTable { operation: operation.to_string(), argument_count: items.len() - 1 }, span)?;
        Ok(())
    }

    pub(crate) fn compile_character_element(
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
            Instruction::CharacterElement {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_character_comparison(
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
            Instruction::CharacterComparison {
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
        Self::require_arity(items, operation, &argument_count.to_string(), argument_count, span)?;
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

#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
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
        if !matches!(items[1].kind, FormKind::Atom(_)) {
            self.emit(function, Instruction::MapInto(Form::list(items.to_vec(), span)), span)?;
            return Ok(());
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
}

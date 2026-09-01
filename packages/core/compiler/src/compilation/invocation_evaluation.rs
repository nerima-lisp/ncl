#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_evaluation_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid = match operation {
            "MAKE-INSTANCE" => items.len() >= 2,
            "COMPILE" => (2..=3).contains(&items.len()),
            "LOAD" => items.len() == 2,
            "PROVIDE" => items.len() == 2,
            "REQUIRE" => (2..=3).contains(&items.len()),
            _ => false,
        };
        if !valid {
            let expected = match operation {
                "MAKE-INSTANCE" => "at least one",
                "COMPILE" => "one or two",
                "LOAD" => "one",
                "PROVIDE" => "one",
                "REQUIRE" => "one or two",
                _ => "valid arguments",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::EvaluationOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

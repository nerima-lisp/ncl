#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_hash_table(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "GETHASH" => ((3..=4).contains(&items.len()), "two or three"),
            "REMHASH" => (items.len() == 3, "two"),
            "MAKE-HASH-TABLE" => ((items.len() - 1).is_multiple_of(2), "keyword/value pairs"),
            "CLRHASH"
            | "HASH-TABLE-COUNT"
            | "HASH-TABLE-SIZE"
            | "HASH-TABLE-TEST"
            | "NCL-HASH-TABLE-KEYS"
            | "NCL-HASH-TABLE-VALUES" => (items.len() == 2, "one"),
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
            Instruction::HashTable {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

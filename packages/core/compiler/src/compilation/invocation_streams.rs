#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_stream_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid = match operation {
            "TERPRI" | "FRESH-LINE" | "FORCE-OUTPUT" | "FINISH-OUTPUT" | "CLEAR-OUTPUT" => {
                items.len() <= 2
            }
            "WRITE-CHAR" => (2..=3).contains(&items.len()),
            "WRITE-STRING" | "WRITE-LINE" | "WRITE-SEQUENCE" => items.len() >= 2,
            "READ-SEQUENCE" => items.len() >= 3,
            "READ-BYTE" => (2..=5).contains(&items.len()),
            "WRITE-BYTE" => items.len() == 3,
            "LISTEN" | "READ-CHAR-NO-HANG" | "CLEAR-INPUT" => items.len() <= 2,
            "PRINC" | "PRIN1" | "PRINT" => (2..=3).contains(&items.len()),
            "WRITE" => items.len() >= 2,
            "GET-OUTPUT-STREAM-STRING" => items.len() == 2,
            "READ-CHAR" | "READ-LINE" => items.len() <= 5,
            "PEEK-CHAR" => items.len() <= 6,
            "UNREAD-CHAR" => (2..=3).contains(&items.len()),
            "CLOSE" => items.len() == 2 || items.len() == 4,
            "STREAM-ELEMENT-TYPE" | "STREAM-EXTERNAL-FORMAT" | "FILE-LENGTH" => items.len() == 2,
            "FILE-POSITION" => (2..=3).contains(&items.len()),
            "MAKE-STRING-INPUT-STREAM" => {
                items.len() >= 2 && (items.len() <= 4 || (items.len() - 2).is_multiple_of(2))
            }
            "MAKE-STRING-OUTPUT-STREAM" => items.len() == 1,
            "WRITE-TO-STRING" | "READ-FROM-STRING" => items.len() >= 2,
            "READ" | "READ-PRESERVING-WHITESPACE" => (1..=5).contains(&items.len()),
            _ => false,
        };
        if !valid {
            return Err(Self::arity_error(
                items,
                operation,
                "the supported argument count",
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::StreamOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_file_operation(
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
                "a pathname and keyword/value pairs",
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::FileOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_file_metadata_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let expected = if operation == "RENAME-FILE" { 2 } else { 1 };
        if items.len() != expected + 1 {
            return Err(Self::arity_error(
                items,
                operation,
                &format!("exactly {expected} argument(s)"),
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::FileMetadataOperation {
                operation: operation.to_string(),
                argument_count: expected,
            },
            span,
        )?;
        Ok(())
    }
}

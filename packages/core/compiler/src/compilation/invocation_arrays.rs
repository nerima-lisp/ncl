use super::*;

impl CompileState {
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

    pub(crate) fn compile_array_adjustment(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "ADJUST-ARRAY",
                "at least two",
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ArrayAdjustment {
                argument_count: items.len() - 1,
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
}

use super::*;

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
                "FLOAT" | "FLOAT-SIGN" | "LOG" | "ATAN" | "COMPLEX" => "one or two",
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

    pub(crate) fn compile_equality(
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
            Instruction::Equality {
                operation: operation.to_string(),
            },
            span,
        )?;
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
        self.emit(
            function,
            Instruction::NumericUnary {
                operation: operation.to_string(),
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_numeric_random(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let argument_count = items.len() - 1;
        if !(1..=2).contains(&argument_count) {
            return Err(Self::arity_error(items, "RANDOM", "one or two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::NumericRandom { argument_count },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_numeric_rounding(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let argument_count = items.len() - 1;
        if !(1..=2).contains(&argument_count) {
            return Err(Self::arity_error(items, operation, "one or two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::NumericRounding {
                operation: operation.to_string(),
                argument_count,
            },
            span,
        )?;
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
        self.emit(
            function,
            Instruction::NumericComparison {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
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
        self.emit(
            function,
            Instruction::NumericFold {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
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
        self.emit(
            function,
            Instruction::NumericBinary {
                operation: operation.to_string(),
            },
            span,
        )?;
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
            Instruction::NumericBitfield {
                operation: operation.to_string(),
                argument_count,
            },
            span,
        )?;
        Ok(())
    }
}

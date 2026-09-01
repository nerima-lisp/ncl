#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_property_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = match operation {
            "GETF" | "GET" => (3..=4).contains(&items.len()),
            "GET-PROPERTIES" => items.len() == 3,
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
        self.emit(
            function,
            Instruction::PropertyList {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_symbol_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = if operation == "CONSTANTP" {
            (2..=3).contains(&items.len())
        } else {
            items.len() == 2
        };
        if !valid_arity {
            let expected = if operation == "CONSTANTP" {
                "one or two"
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
            Instruction::SymbolValue {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_symbol_binding(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "SET" => (items.len() == 3, "two"),
            "MAKUNBOUND" | "FMAKUNBOUND" => (items.len() == 2, "one"),
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
            Instruction::SymbolBinding {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_symbol_function(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid_arity = if operation == "MACRO-FUNCTION" {
            (2..=3).contains(&items.len())
        } else {
            items.len() == 2
        };
        if !valid_arity {
            let expected = if operation == "MACRO-FUNCTION" {
                "one or two"
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
            Instruction::SymbolFunction {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_symbol_creation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "MAKE-SYMBOL" => (items.len() == 2, "one"),
            "GENSYM" => ((1..=2).contains(&items.len()), "zero or one"),
            "INTERN" | "FIND-SYMBOL" => ((2..=3).contains(&items.len()), "one or two"),
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
            Instruction::SymbolCreation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

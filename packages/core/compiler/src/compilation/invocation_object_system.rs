#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_class_introspection(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let (valid_arity, expected) = match operation {
            "SUBTYPEP" => (items.len() == 3, "two"),
            "CLASS-OF"
            | "CLASS-NAME"
            | "CLASS-DOCUMENTATION"
            | "CLASS-PRECEDENCE-LIST"
            | "CLASS-DIRECT-SUPERCLASSES"
            | "CLASS-DIRECT-SLOTS"
            | "CLASS-SLOTS"
            | "CLASS-DEFAULT-INITARGS"
            | "CLASS-DIRECT-DEFAULT-INITARGS"
            | "CLASS-FINALIZED-P"
            | "FINALIZE-INHERITANCE" => (items.len() == 2, "one"),
            "GENERIC-FUNCTION-NAME"
            | "GENERIC-FUNCTION-METHOD-COMBINATION"
            | "GENERIC-FUNCTION-LAMBDA-LIST"
            | "GENERIC-FUNCTION-DOCUMENTATION"
            | "GENERIC-FUNCTION-METHODS"
            | "METHOD-QUALIFIERS"
            | "METHOD-SPECIALIZERS"
            | "METHOD-FUNCTION" => (items.len() == 2, "one"),
            "FIND-METHOD" => (items.len() == 4, "three"),
            "ADD-METHOD" | "REMOVE-METHOD" => (items.len() == 3, "two"),
            "ENSURE-GENERIC-FUNCTION" => (items.len() >= 2, "at least one"),
            "FIND-CLASS" => ((2..=3).contains(&items.len()), "one or two"),
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
            Instruction::ClassIntrospection {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_slot_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let expected_arguments = if matches!(
            operation,
            "SLOT-DEFINITION-NAME"
                | "SLOT-DEFINITION-DOCUMENTATION"
                | "SLOT-DEFINITION-INITARGS"
                | "SLOT-DEFINITION-ALLOCATION"
                | "SLOT-DEFINITION-INITFORM"
                | "SLOT-DEFINITION-TYPE"
        ) {
            1
        } else {
            2
        };
        if items.len() != expected_arguments + 1 {
            return Err(Self::arity_error(
                items,
                operation,
                if expected_arguments == 1 {
                    "one"
                } else {
                    "two"
                },
                span,
            ));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::SlotOperation {
                operation: operation.to_string(),
                argument_count: expected_arguments,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_condition_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, operation, "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::ConditionOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_restart_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid = match operation {
            "COMPUTE-RESTARTS" => (1..=2).contains(&items.len()),
            "RESTART-NAME" => items.len() == 2,
            "FIND-RESTART" => (2..=3).contains(&items.len()),
            "INVOKE-RESTART" => items.len() >= 2,
            _ => false,
        };
        if !valid {
            let expected = match operation {
                "COMPUTE-RESTARTS" => "zero or one",
                "RESTART-NAME" => "one",
                "FIND-RESTART" => "one or two",
                "INVOKE-RESTART" => "at least one",
                _ => "valid arguments",
            };
            return Err(Self::arity_error(items, operation, expected, span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::RestartOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_method_operation(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operation: &str,
    ) -> Result<(), CompileError> {
        let valid = match operation {
            "CALL-NEXT-METHOD" => true,
            "NEXT-METHOD-P" => items.len() == 1,
            _ => false,
        };
        if !valid {
            return Err(Self::arity_error(items, operation, "zero", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MethodOperation {
                operation: operation.to_string(),
                argument_count: items.len() - 1,
            },
            span,
        )?;
        Ok(())
    }
}

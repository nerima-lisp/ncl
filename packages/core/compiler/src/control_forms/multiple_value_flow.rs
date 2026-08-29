#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_multiple_value_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "MULTIPLE-VALUE-BIND",
                "at least two",
                span,
            ));
        }
        let Some(variable_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-BIND variables",
            ));
        };
        let FormKind::List(variables) = &variable_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "MULTIPLE-VALUE-BIND variables".to_string(),
                },
                variable_form.span,
            ));
        };
        let mut names = Vec::with_capacity(variables.len());
        for variable in variables {
            names.push(Self::symbol_name_info(
                variable,
                "MULTIPLE-VALUE-BIND variable",
            )?);
        }
        let Some(value_form) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-BIND value form",
            ));
        };

        self.emit(function, Instruction::EnterScope, variable_form.span)?;
        self.compile_expression(function, value_form)?;
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::BindValuesExact(names)
        } else {
            Instruction::BindValues(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, value_form.span)?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(crate) fn compile_multiple_value_call(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "MULTIPLE-VALUE-CALL",
                "at least one",
                span,
            ));
        }
        let Some(function_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-CALL function",
            ));
        };
        self.compile_expression(function, function_form)?;
        self.emit(function, Instruction::Primary, function_form.span)?;
        for value_form in items.get(2..).unwrap_or(&[]) {
            self.compile_expression(function, value_form)?;
        }
        self.emit(
            function,
            Instruction::MultipleValueCall(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_multiple_value_prog1(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "MULTIPLE-VALUE-PROG1",
                "at least one",
                span,
            ));
        }
        let Some(first) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-PROG1 form after arity check",
            ));
        };
        let retained = self.fresh_name("MULTIPLE_VALUE_PROG1_VALUE");

        self.emit(function, Instruction::EnterScope, first.span)?;
        self.compile_expression(function, first)?;
        self.emit(
            function,
            Instruction::DefineValues(retained.clone()),
            first.span,
        )?;
        self.emit(function, Instruction::Pop, first.span)?;

        let tail = items.get(2..).unwrap_or(&[]);
        if !tail.is_empty() {
            self.compile_sequence(function, tail)?;
            self.emit(function, Instruction::Pop, span)?;
        }

        self.emit(function, Instruction::Load(retained), span)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

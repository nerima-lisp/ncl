#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_ignore_errors(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        self.compile_sequence(child, &items[1..])?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::IgnoreErrors(child), span)?;
        Ok(())
    }

    pub(crate) fn compile_handler_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "HANDLER-CASE",
                "at least two",
                span,
            ));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing HANDLER-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case clause needs a condition and variable list"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let condition = Self::condition_name(&clause_items[0], "handler-case condition")?;
            let FormKind::List(variable_items) = &clause_items[1].kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-case variable list".to_string(),
                    },
                    clause_items[1].span,
                ));
            };
            if variable_items.len() > 1 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-case variable list accepts at most one variable"
                            .to_string(),
                    },
                    clause_items[1].span,
                ));
            }
            let variable = variable_items
                .first()
                .map(|form| Self::symbol_name_info(form, "handler-case variable"))
                .transpose()?;
            let parameters = variable
                .as_ref()
                .map(|(name, _)| name.clone())
                .into_iter()
                .collect();
            let required_escaped = variable
                .as_ref()
                .map(|(_, escaped)| *escaped)
                .into_iter()
                .collect();
            let clause_function =
                self.reserve_function_with_rest(None, parameters, required_escaped, None, false);
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(HandlerCaseClause {
                condition,
                variable: variable.map(|(name, _)| name),
                function: clause_function,
            });
        }

        self.emit(
            function,
            Instruction::HandlerCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }
}

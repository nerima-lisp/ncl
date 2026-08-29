#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_restart_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(
                items,
                "RESTART-CASE",
                "at least two",
                span,
            ));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing RESTART-CASE protected form"))?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-case clause".to_string(),
                    },
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-case clause needs a name, lambda list, and body"
                            .to_string(),
                    },
                    clause.span,
                ));
            }
            let name = Self::control_name(&clause_items[0], "RESTART-CASE restart name")?;
            let lambda_list = Self::parameters(&clause_items[1])?;
            let clause_function = self.reserve_function_with_rest(
                None,
                lambda_list.required.clone(),
                lambda_list.required_escaped.clone(),
                lambda_list.rest.clone(),
                lambda_list.rest_escaped,
            );
            let optional = self.compile_optional_parameters(&lambda_list.optional)?;
            self.functions[clause_function].optional = optional;
            let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
            self.functions[clause_function].keywords = keywords;
            self.functions[clause_function].has_keyword_section = lambda_list.has_keyword_section;
            self.functions[clause_function].allow_other_keys = lambda_list.allow_other_keys;
            let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
            self.functions[clause_function].auxiliary = auxiliary;
            self.compile_sequence(clause_function, &clause_items[2..])?;
            self.emit(clause_function, Instruction::Return, clause.span)?;
            clauses.push(RestartCaseClause {
                name,
                function: clause_function,
            });
        }

        self.emit(
            function,
            Instruction::RestartCase {
                protected: protected_function,
                clauses,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_with_condition_restarts(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "WITH-CONDITION-RESTARTS",
                "at least three",
                span,
            ));
        }

        let condition = self.reserve_function(None, Vec::new());
        self.compile_expression(condition, &items[1])?;
        self.emit(condition, Instruction::Return, items[1].span)?;

        let restarts = self.reserve_function(None, Vec::new());
        self.compile_expression(restarts, &items[2])?;
        self.emit(restarts, Instruction::Return, items[2].span)?;

        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, &items[3..])?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithConditionRestarts {
                condition,
                restarts,
                body,
            },
            span,
        )?;
        Ok(())
    }
}

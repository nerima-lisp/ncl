use super::*;

impl CompileState {
    pub(super) fn compile_ignore_errors(
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

    pub(super) fn compile_handler_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "HANDLER-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-CASE protected form"))?;
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
            let condition = self.condition_name(&clause_items[0], "handler-case condition")?;
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
                .map(|form| self.symbol_name_info(form, "handler-case variable"))
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

    pub(super) fn compile_handler_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "HANDLER-BIND", "at least one", span));
        }
        let handler_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing HANDLER-BIND handler list"))?;
        let FormKind::List(handler_items) = &handler_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "handler-bind handler list".to_string(),
                },
                handler_form.span,
            ));
        };

        let mut handlers = Vec::with_capacity(handler_items.len());
        for handler in handler_items {
            let FormKind::List(handler_clause) = &handler.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "handler-bind clause".to_string(),
                    },
                    handler.span,
                ));
            };
            if handler_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "handler-bind clause needs a condition and handler".to_string(),
                    },
                    handler.span,
                ));
            }
            let condition = self.condition_name(&handler_clause[0], "handler-bind condition")?;
            let condition_variable = self.fresh_name("HANDLER_CONDITION");
            let clause_function = self.reserve_function(None, vec![condition_variable.clone()]);
            self.compile_expression(clause_function, &handler_clause[1])?;
            self.compile_expression(
                clause_function,
                &Form::atom(condition_variable, handler_clause[1].span),
            )?;
            self.emit(
                clause_function,
                Instruction::Call(1),
                handler_clause[1].span,
            )?;
            self.emit(clause_function, Instruction::Return, handler.span)?;
            handlers.push(HandlerBindClause {
                condition,
                function: clause_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::HandlerBind {
                body: body_function,
                handlers,
            },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_restart_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "RESTART-BIND", "at least one", span));
        }
        let binding_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-BIND binding list"))?;
        let FormKind::List(binding_items) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "restart-bind binding list".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut bindings = Vec::with_capacity(binding_items.len());
        for binding in binding_items {
            let FormKind::List(binding_clause) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "restart-bind clause".to_string(),
                    },
                    binding.span,
                ));
            };
            if binding_clause.len() != 2 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "restart-bind clause needs a name and function".to_string(),
                    },
                    binding.span,
                ));
            }
            let name = self.control_name(&binding_clause[0], "RESTART-BIND restart name")?;
            let binding_function = self.reserve_function(None, Vec::new());
            self.compile_expression(binding_function, &binding_clause[1])?;
            self.emit(
                binding_function,
                Instruction::Return,
                binding_clause[1].span,
            )?;
            bindings.push(RestartBindClause {
                name,
                function: binding_function,
            });
        }

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::RestartBind {
                body: body_function,
                bindings,
            },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_catch(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "CATCH", "at least one", span));
        }

        let tag_function = self.reserve_function(None, Vec::new());
        self.compile_expression(tag_function, &items[1])?;
        self.emit(tag_function, Instruction::Return, items[1].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Catch {
                tag: tag_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_with_simple_restart(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "WITH-SIMPLE-RESTART", "at least one", span));
        }

        let clause = &items[1];
        let FormKind::List(parts) = &clause.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "WITH-SIMPLE-RESTART restart clause".to_string(),
                },
                clause.span,
            ));
        };
        if parts.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "WITH-SIMPLE-RESTART restart clause needs a name and report format"
                        .to_string(),
                },
                clause.span,
            ));
        }

        let name = self.control_name(&parts[0], "WITH-SIMPLE-RESTART name")?;
        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, items.get(2..).unwrap_or(&[]))?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::WithSimpleRestart { name, body },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_restart_case(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "RESTART-CASE", "at least two", span));
        }

        let protected = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing RESTART-CASE protected form"))?;
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
            let name = self.control_name(&clause_items[0], "RESTART-CASE restart name")?;
            let lambda_list = self.parameters(&clause_items[1])?;
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

    pub(super) fn compile_with_condition_restarts(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(self.arity_error(items, "WITH-CONDITION-RESTARTS", "at least three", span));
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

    pub(super) fn compile_throw(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(self.arity_error(items, "THROW", "two", span));
        }

        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Throw, span)?;
        Ok(())
    }

    pub(super) fn compile_progv(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "PROGV", "at least two", span));
        }

        let symbols_function = self.reserve_function(None, Vec::new());
        self.compile_expression(symbols_function, &items[1])?;
        self.emit(symbols_function, Instruction::Return, items[1].span)?;

        let values_function = self.reserve_function(None, Vec::new());
        self.compile_expression(values_function, &items[2])?;
        self.emit(values_function, Instruction::Return, items[2].span)?;

        let body_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(body_function, items.get(3..).unwrap_or(&[]))?;
        self.emit(body_function, Instruction::Return, span)?;

        self.emit(
            function,
            Instruction::Progv {
                symbols: symbols_function,
                values: values_function,
                body: body_function,
            },
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_unwind_protect(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "UNWIND-PROTECT", "at least one", span));
        }

        let protected = items.get(1).ok_or_else(|| {
            self.internal_error(
                span,
                "missing UNWIND-PROTECT protected form after arity check",
            )
        })?;
        let protected_function = self.reserve_function(None, Vec::new());
        self.compile_expression(protected_function, protected)?;
        self.emit(protected_function, Instruction::Return, protected.span)?;

        let cleanup_function = self.reserve_function(None, Vec::new());
        self.compile_sequence(cleanup_function, items.get(2..).unwrap_or(&[]))?;
        self.emit(cleanup_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::UnwindProtect {
                protected: protected_function,
                cleanup: cleanup_function,
            },
            span,
        )?;
        Ok(())
    }
}

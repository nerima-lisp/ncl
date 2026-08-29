#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_restart_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "RESTART-BIND",
                "at least one",
                span,
            ));
        }
        let binding_form = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing RESTART-BIND binding list"))?;
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
            let name = Self::control_name(&binding_clause[0], "RESTART-BIND restart name")?;
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

    pub(crate) fn compile_with_simple_restart(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "WITH-SIMPLE-RESTART",
                "at least one",
                span,
            ));
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

        let name = Self::control_name(&parts[0], "WITH-SIMPLE-RESTART name")?;
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
}

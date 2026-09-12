#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_funcall(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "FUNCALL", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_eval(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "EVAL", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing eval argument after arity check",
            ));
        };
        self.compile_expression(function, argument)?;
        self.emit(function, Instruction::Eval(argument.span), span)?;
        Ok(())
    }

    pub(crate) fn compile_apply(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "APPLY", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Apply(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_mapcar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAPCAR", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MapCar(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_map_into(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAP-INTO", "at least two", span));
        }
        let destination = items[1].clone();
        let stable_list_destination = match &destination.kind {
            FormKind::List(destination_items) => {
                destination_items.len() == 2
                    && Self::symbol_name_info(&destination_items[0], "list place operator")
                        .is_ok_and(|(name, escaped)| {
                            !escaped && matches!(name.as_str(), "CAR" | "CDR" | "FIRST" | "REST")
                        })
                    && matches!(destination_items[1].kind, FormKind::Atom(_))
            }
            _ => false,
        };
        if stable_list_destination {
            let FormKind::List(destination_items) = &destination.kind else {
                unreachable!()
            };
            let operator = Self::symbol_name_info(&destination_items[0], "list place operator")?.0;
            self.compile_expression(function, &destination_items[1])?;
            self.compile_expression(function, &items[2])?;
            for item in &items[3..] {
                self.compile_expression(function, item)?;
            }
            self.emit(
                function,
                Instruction::MapIntoListPlace {
                    operator,
                    place: destination_items[1].clone(),
                    sequence_count: items.len().saturating_sub(3),
                },
                span,
            )?;
            return Ok(());
        }
        let stable_nth_destination = match &destination.kind {
            FormKind::List(destination_items) => {
                destination_items.len() == 3
                    && Self::symbol_name_info(&destination_items[0], "NTH place operator")
                        .is_ok_and(|(name, escaped)| !escaped && name == "NTH")
                    && matches!(destination_items[2].kind, FormKind::Atom(_))
            }
            _ => false,
        };
        if stable_nth_destination {
            let FormKind::List(destination_items) = &destination.kind else {
                unreachable!()
            };
            self.compile_expression(function, &destination_items[1])?;
            self.compile_expression(function, &destination_items[2])?;
            self.compile_expression(function, &items[2])?;
            for item in &items[3..] {
                self.compile_expression(function, item)?;
            }
            self.emit(
                function,
                Instruction::MapIntoNthPlace {
                    sequence_count: items.len().saturating_sub(3),
                    place: destination,
                },
                span,
            )?;
            return Ok(());
        }
        let stable_vector_destination = match &destination.kind {
            FormKind::List(destination_items) => {
                destination_items.len() == 3
                    && Self::symbol_name_info(&destination_items[0], "AREF place operator")
                        .is_ok_and(|(name, escaped)| !escaped && name == "AREF")
                    && matches!(destination_items[1].kind, FormKind::Atom(_))
            }
            _ => false,
        };
        if stable_vector_destination {
            let FormKind::List(destination_items) = &destination.kind else {
                unreachable!()
            };
            self.compile_expression(function, &destination_items[2])?;
            self.compile_expression(function, &destination_items[1])?;
            self.compile_expression(function, &items[2])?;
            for item in &items[3..] {
                self.compile_expression(function, item)?;
            }
            self.emit(
                function,
                Instruction::MapIntoArefVectorPlace {
                    sequence_count: items.len().saturating_sub(3),
                    place: destination,
                },
                span,
            )?;
            return Ok(());
        }
        if let Some(form) = items[1..]
            .iter()
            .find(|form| matches!(form.kind, FormKind::DottedList { .. }))
        {
            return Err(CompileError::new(
                CompileErrorKind::UnsupportedForm {
                    message: "dotted lists cannot be evaluated".to_string(),
                },
                form.span,
            ));
        }
        self.compile_runtime_definition(function, span, items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dotted(span: Span) -> Form {
        Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
    }

    #[test]
    fn compile_funcall_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![Form::atom("FUNCALL", span), dotted(span)];

        let error = state.compile_funcall(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_funcall_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("FUNCALL", span), Form::atom("F", span)];

        let error = state.compile_funcall(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_eval_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![Form::atom("EVAL", span), dotted(span)];

        let error = state.compile_eval(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_apply_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("APPLY", span),
            Form::atom("F", span),
            dotted(span),
        ];

        let error = state.compile_apply(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_mapcar_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("MAPCAR", span),
            Form::atom("F", span),
            dotted(span),
        ];

        let error = state.compile_mapcar(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_map_into_propagates_an_argument_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("MAP-INTO", span),
            Form::atom("D", span),
            dotted(span),
        ];

        let error = state.compile_map_into(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_map_into_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("MAP-INTO", span),
            Form::atom("D", span),
            Form::atom("S", span),
        ];

        let error = state.compile_map_into(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }
}

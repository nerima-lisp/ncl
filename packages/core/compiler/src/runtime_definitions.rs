#[allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(super) fn compile_load_time_value(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(
                items,
                "LOAD-TIME-VALUE",
                "one or two",
                span,
            ));
        }
        self.compile_runtime_definition(function, span, items)
    }

    pub(super) fn compile_defstruct(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DEFSTRUCT", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    pub(super) fn compile_runtime_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(
                items,
                "runtime definition",
                "at least one",
                span,
            ));
        }
        if let Some(result) = self.compile_native_push_pop(function, span, items)? {
            return Ok(result);
        }
        if let Some(result) = self.compile_native_rotate_shift(function, span, items)? {
            return Ok(result);
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    fn compile_native_rotate_shift(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        let Some((operator, _)) = items
            .first()
            .and_then(|form| Self::symbol_name_info(form, "runtime operator").ok())
        else {
            return Ok(None);
        };
        if operator != "ROTATEF" && operator != "SHIFTF" {
            return Ok(None);
        }
        let place_count = if operator == "ROTATEF" {
            items.len().saturating_sub(1)
        } else {
            items.len().saturating_sub(2)
        };
        if operator == "ROTATEF" && place_count < 2 {
            return Ok(None);
        }
        if operator == "SHIFTF" && place_count < 1 {
            return Err(Self::arity_error(items, &operator, "at least one", span));
        }
        let place_forms = if operator == "ROTATEF" {
            &items[1..]
        } else {
            &items[1..items.len() - 1]
        };
        let places = place_forms
            .iter()
            .map(|place| Self::symbol_name_info(place, "symbol place"))
            .collect::<Result<Vec<_>, _>>()
            .ok();
        let Some(places) = places else {
            return Ok(None);
        };
        for place in place_forms {
            self.compile_expression(function, place)?;
        }
        if operator == "SHIFTF" {
            self.compile_expression(function, &items[items.len() - 1])?;
        }
        self.emit(
            function,
            if operator == "ROTATEF" {
                Instruction::RotatefSymbols(places)
            } else {
                Instruction::ShiftfSymbols(places)
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }

    fn compile_native_push_pop(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        let Some(operator) = items
            .first()
            .and_then(|form| Self::symbol_name_info(form, "runtime operator").ok())
            .map(|(name, _)| name)
        else {
            return Ok(None);
        };
        if !matches!(operator.as_str(), "PUSH" | "POP" | "PUSHNEW") {
            return Ok(None);
        }
        let expected = if operator == "POP" { 2 } else { 3 };
        if operator == "PUSHNEW" && items.len() > expected {
            return Ok(None);
        }
        if items.len() != expected {
            return Err(Self::arity_error(
                items,
                &operator,
                if operator == "PUSH" { "two" } else { "one" },
                span,
            ));
        }
        let Some((name, escaped)) = Self::symbol_name_info(&items[expected - 1], "list place").ok()
        else {
            return Ok(None);
        };
        if matches!(operator.as_str(), "PUSH" | "PUSHNEW") {
            self.compile_expression(function, &items[1])?;
        }
        self.compile_expression(function, &items[expected - 1])?;
        self.emit(
            function,
            match operator.as_str() {
                "PUSH" => Instruction::PushList { name, escaped },
                "PUSHNEW" => Instruction::PushNewList { name, escaped },
                _ => Instruction::PopList { name, escaped },
            },
            items[0].span,
        )?;
        Ok(Some(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_items(source: &str) -> Vec<Form> {
        let mut forms = ncl_syntax::read(source).expect("test source should parse");
        match forms.remove(0).kind {
            ncl_syntax::FormKind::List(items) => items,
            form => panic!("expected list form, got {form:?}"),
        }
    }

    #[test]
    fn compile_defstruct_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("DEFSTRUCT", span), Form::atom("POINT", span)];

        let error = state.compile_defstruct(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_runtime_definition_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("DEFPACKAGE", span), Form::atom("FOO", span)];

        let error = state
            .compile_runtime_definition(99, span, &items)
            .map_or_else(
                |error| error,
                |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_load_time_value_rejects_more_than_two_arguments() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("LOAD-TIME-VALUE", span),
            Form::atom("1", span),
            Form::atom("NIL", span),
            Form::atom("NIL", span),
        ];

        let Err(error) = state.compile_load_time_value(function, span, &items) else {
            panic!("too many LOAD-TIME-VALUE arguments must fail during compilation")
        };

        assert!(matches!(error.kind, CompileErrorKind::Arity { .. }));
    }

    #[test]
    fn compile_runtime_definition_uses_native_rotate_and_shift_for_symbol_places() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let rotatef = parse_items("(rotatef a |B| c)");
        state
            .compile_runtime_definition(function, Span::new(0, 1), &rotatef)
            .expect("ROTATEF symbol places should compile");
        assert!(
            state.functions[function]
                .instructions
                .contains(&Instruction::RotatefSymbols(vec![
                    ("A".to_string(), false),
                    ("B".to_string(), true),
                    ("C".to_string(), false),
                ]))
        );

        let shiftf = parse_items("(shiftf a b 9)");
        state
            .compile_runtime_definition(function, Span::new(0, 1), &shiftf)
            .expect("SHIFTF symbol places should compile");
        assert!(
            state.functions[function]
                .instructions
                .contains(&Instruction::ShiftfSymbols(vec![
                    ("A".to_string(), false),
                    ("B".to_string(), false),
                ]))
        );
    }

    #[test]
    fn compile_runtime_definition_falls_back_for_generalized_rotate_and_shift_places() {
        let mut state = CompileState::default();
        for source in ["(rotatef (car xs) y)", "(shiftf (car xs) y 9)"] {
            let function = state.reserve_function(None, Vec::new());
            let items = parse_items(source);
            state
                .compile_runtime_definition(function, Span::new(0, 1), &items)
                .expect("generalized places should use evaluator fallback");
            assert!(
                state.functions[function]
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, Instruction::Eval(_)))
            );
            assert!(
                !state.functions[function]
                    .instructions
                    .iter()
                    .any(|instruction| {
                        matches!(
                            instruction,
                            Instruction::RotatefSymbols(_) | Instruction::ShiftfSymbols(_)
                        )
                    })
            );
        }
    }

    #[test]
    fn compile_runtime_definition_falls_back_for_single_place_rotatef() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let items = parse_items("(rotatef a)");

        state
            .compile_runtime_definition(function, Span::new(0, 1), &items)
            .expect("single-place ROTATEF should use evaluator fallback");
        assert!(
            state.functions[function]
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Eval(_)))
        );
    }
}

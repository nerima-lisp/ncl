use crate::{
    CompileError, CompileErrorKind, CompileState, Constant, Form, FormKind, FunctionId,
    Instruction, Span, literal_constant, normalize_name, symbol_reference,
};

impl CompileState {
    pub(crate) fn compile_sequence(
        &mut self,
        function: FunctionId,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        if forms.is_empty() {
            self.emit(
                function,
                Instruction::Constant(Constant::Nil),
                Span::new(0, 0),
            )?;
            return Ok(());
        }

        for (index, form) in forms.iter().enumerate() {
            self.compile_expression(function, form)?;
            if index + 1 < forms.len() {
                self.emit(function, Instruction::Pop, form.span)?;
            }
        }
        Ok(())
    }

    pub(crate) fn compile_expression(
        &mut self,
        function: FunctionId,
        form: &Form,
    ) -> Result<(), CompileError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(constant) = literal_constant(atom) {
                    self.emit(function, Instruction::Constant(constant), form.span)?;
                } else if let Some((name, escaped)) = symbol_reference(atom) {
                    let instruction = if escaped {
                        Instruction::LoadExact(name)
                    } else {
                        Instruction::Load(name)
                    };
                    self.emit(function, instruction, form.span)?;
                } else {
                    self.emit(function, Instruction::Load(normalize_name(atom)), form.span)?;
                }
            }
            FormKind::String(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::String(value.clone())),
                    form.span,
                )?;
            }
            FormKind::Character(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::Character(*value)),
                    form.span,
                )?;
            }
            FormKind::Vector(_) => {
                self.emit(function, Instruction::Quote(form.clone()), form.span)?;
            }
            FormKind::DottedList { .. } => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedForm {
                        message: "dotted lists cannot be evaluated".to_string(),
                    },
                    form.span,
                ));
            }
            FormKind::List(items) => self.compile_list(function, form.span, items)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_expression_rejects_dotted_lists_as_unsupported() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let form = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);

        let error = state.compile_expression(function, &form).map_or_else(
            |error| error,
            |value| panic!("dotted lists cannot be evaluated, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { message } if message == "dotted lists cannot be evaluated"
        ));
    }

    #[test]
    fn compile_expression_falls_back_to_load_for_unclassified_qualified_symbols() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let form = Form::atom("FOO:|bar|", span);

        state
            .compile_expression(function, &form)
            .unwrap_or_else(|error| {
                panic!("a qualified escaped symbol still compiles to a Load: {error}")
            });

        assert_eq!(
            state.functions[function].instructions,
            vec![Instruction::Load(normalize_name("FOO:|bar|"))]
        );
    }

    #[test]
    fn compile_sequence_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 0);

        let error = state.compile_sequence(42, &[]).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
        assert_eq!(error.span, span);
    }
}

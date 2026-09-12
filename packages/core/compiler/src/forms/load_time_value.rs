use crate::{CompileError, CompileState, Form, FunctionId, Instruction, Span};

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
        self.emit(
            function,
            Instruction::LoadTimeValue(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Instruction;

    #[test]
    fn compiles_load_time_value_as_a_native_instruction() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let items = vec![Form::atom("LOAD-TIME-VALUE", span), Form::atom("42", span)];

        if let Err(error) = state.compile_load_time_value(function, span, &items) {
            panic!("literal LOAD-TIME-VALUE should compile: {error}");
        }

        assert_eq!(
            state.functions[function].instructions,
            vec![Instruction::LoadTimeValue(Form::list(items, span))]
        );
    }

    #[test]
    fn preserves_computed_load_time_value_forms() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let value = Form::list(
            vec![
                Form::atom("+", span),
                Form::atom("1", span),
                Form::atom("2", span),
            ],
            span,
        );
        let items = vec![Form::atom("LOAD-TIME-VALUE", span), value];

        if let Err(error) = state.compile_load_time_value(function, span, &items) {
            panic!("computed LOAD-TIME-VALUE should compile: {error}");
        }
        assert_eq!(
            state.functions[function].instructions,
            vec![Instruction::LoadTimeValue(Form::list(items, span))]
        );
    }
}

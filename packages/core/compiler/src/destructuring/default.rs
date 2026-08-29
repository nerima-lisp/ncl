#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_default(
        &mut self,
        form: &Form,
    ) -> Result<FunctionId, CompileError> {
        let default_function = self.reserve_function(None, Vec::new());
        self.compile_expression(default_function, form)?;
        self.emit(default_function, Instruction::Return, form.span)?;
        Ok(default_function)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_destructuring_default_propagates_an_expression_error() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);

        let error = state.compile_destructuring_default(&dotted).map_or_else(
            |error| error,
            |value| panic!("a malformed default value should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }
}

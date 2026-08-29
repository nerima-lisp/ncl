#[cfg(test)]
use crate::CompileErrorKind;
use crate::{
    CompileError, CompileState, Constant, Form, FormKind, FunctionId, Instruction, Span,
    normalize_name, special_operator_name, symbol_reference,
};

impl CompileState {
    pub(super) fn compile_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let Some(operator) = items.first() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let operator_name = match &operator.kind {
            FormKind::Atom(name) => special_operator_name(name),
            _ => None,
        };
        if let Some(name) = operator_name.as_deref() {
            if let Some(result) = self.dispatch_core_and_control_forms(name, function, span, items)
            {
                return result;
            }
            if let Some(result) = self.dispatch_logic_and_binding_forms(name, function, span, items)
            {
                return result;
            }
        }

        self.compile_call(function, span, operator, items)
    }

    fn compile_call(
        &mut self,
        function: FunctionId,
        span: Span,
        operator: &Form,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if let FormKind::Atom(name) = &operator.kind {
            let (reference_name, escaped) =
                symbol_reference(name).unwrap_or_else(|| (normalize_name(name), false));
            self.emit(
                function,
                if escaped {
                    Instruction::FunctionLoadExact(reference_name)
                } else {
                    Instruction::FunctionLoad(reference_name)
                },
                operator.span,
            )?;
            for item in items.iter().skip(1) {
                self.compile_expression(function, item)?;
            }
        } else {
            for item in items {
                self.compile_expression(function, item)?;
            }
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_list_compiles_an_empty_list_to_nil() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 0);

        state
            .compile_list(function, span, &[])
            .unwrap_or_else(|error| panic!("an empty list compiles to NIL: {error}"));

        assert_eq!(
            state.functions[function].instructions,
            vec![Instruction::Constant(Constant::Nil)]
        );
    }

    #[test]
    fn compile_call_falls_back_to_normalized_name_for_a_keyword_operator() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let items = vec![Form::atom(":foo", span), Form::atom("1", span)];

        state
            .compile_list(function, span, &items)
            .unwrap_or_else(|error| {
                panic!("a keyword used as an operator still compiles as a call: {error}")
            });

        assert!(
            state.functions[function]
                .instructions
                .contains(&Instruction::FunctionLoad(":FOO".to_string())),
            "expected a normalized FunctionLoad, got {:?}",
            state.functions[function].instructions
        );
    }

    #[test]
    fn compile_call_propagates_an_argument_compilation_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let items = vec![Form::atom("FOO", span), dotted];

        let error = state.compile_list(function, span, &items).map_or_else(
            |error| error,
            |value| panic!("a malformed argument should fail to compile, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_call_propagates_an_error_from_a_non_atom_operator_argument() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let items = vec![Form::list(Vec::new(), span), dotted];

        let error = state
            .compile_list(function, span, &items)
            .map_or_else(|error| error, |value| panic!("a malformed trailing argument should fail even for a non-atom operator, got {value:?}"));

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }
}

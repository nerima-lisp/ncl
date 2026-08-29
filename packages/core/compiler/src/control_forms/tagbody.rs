#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_tagbody(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.compile_tagbody_forms(function, span, items.get(1..).unwrap_or(&[]))
    }

    pub(crate) fn compile_tagbody_forms(
        &mut self,
        function: FunctionId,
        span: Span,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        let child = self.reserve_function(None, Vec::new());
        let mut tags = Vec::new();

        for form in forms {
            if let Some(tag) = tag_name(form) {
                if tags.iter().any(|(existing, _)| existing == &tag) {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: format!("duplicate TAGBODY tag {tag}"),
                        },
                        form.span,
                    ));
                }
                let position = self.instruction_count(child, form.span)?;
                tags.push((tag, position));
            } else {
                self.compile_expression(child, form)?;
                self.emit(child, Instruction::Pop, form.span)?;
            }
        }

        self.emit(child, Instruction::Constant(Constant::Nil), span)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::TagBody {
                function: child,
                tags,
            },
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_go(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "GO", "one", 1, span)?;
        let tag = Self::control_tag(
            items
                .get(1)
                .ok_or_else(|| Self::internal_error(span, "missing GO tag after arity check"))?,
            "GO tag",
        )?;
        self.emit(function, Instruction::Go { tag }, span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_tagbody_forms_propagates_a_body_expression_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);

        let error = state
            .compile_tagbody_forms(function, span, &[dotted])
            .map_or_else(
                |error| error,
                |value| panic!("a malformed body form should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_tagbody_forms_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);

        let error = state.compile_tagbody_forms(99, span, &[]).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn compile_go_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let items = vec![Form::atom("GO", span), Form::atom("DONE", span)];

        let error = state.compile_go(99, span, &items).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }
}

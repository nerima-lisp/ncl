#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_optional_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureOptionalParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (pattern, init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (
                Self::compile_destructuring_pattern(form, seen)?,
                nil(),
                None,
            ),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = Self::compile_destructuring_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| {
                        Self::compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?;
                (pattern, init_form, supplied_p)
            }
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must contain one to three items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureOptionalParameter {
            pattern,
            default_function,
            supplied_p,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad_form(span: Span) -> Form {
        Form::new(FormKind::String("bad".to_string()), span)
    }

    #[test]
    fn compile_destructuring_optional_parameter_rejects_a_non_symbol_non_list_form() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = bad_form(span);

        let error = state
            .compile_destructuring_optional_parameter(&form, &mut seen)
            .map_or_else(
                |error| error,
                |value| panic!("a string literal cannot be an optional parameter, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_optional_parameter_propagates_a_pattern_error() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = Form::list(vec![bad_form(span)], span);

        let error = state
            .compile_destructuring_optional_parameter(&form, &mut seen)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed pattern should fail to compile, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_optional_parameter_propagates_a_supplied_p_error() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = Form::list(
            vec![
                Form::atom("X", span),
                Form::atom("1", span),
                Form::atom(":sp", span),
            ],
            span,
        );

        let error = state
            .compile_destructuring_optional_parameter(&form, &mut seen)
            .map_or_else(
                |error| error,
                |value| panic!("a keyword cannot name a supplied-p variable, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedSymbol { .. }
        ));
    }

    #[test]
    fn compile_destructuring_optional_parameter_propagates_a_default_error() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let form = Form::list(vec![Form::atom("X", span), dotted], span);

        let error = state
            .compile_destructuring_optional_parameter(&form, &mut seen)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed default value should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }
}

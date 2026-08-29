#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_auxiliary_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureAuxiliaryParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (name, init_form) = match &form.kind {
            FormKind::Atom(_) => (
                Self::compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                nil(),
            ),
            FormKind::List(items) if (1..=2).contains(&items.len()) => (
                Self::compile_destructuring_binding_name(
                    &items[0],
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                items.get(1).cloned().unwrap_or_else(nil),
            ),
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must contain one or two items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureAuxiliaryParameter {
            name,
            default_function,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_destructuring_auxiliary_parameter_rejects_a_non_symbol_non_list_form() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = Form::new(FormKind::String("bad".to_string()), span);

        let error = state
            .compile_destructuring_auxiliary_parameter(&form, &mut seen)
            .map_or_else(
                |error| error,
                |value| panic!("a string literal cannot be an auxiliary parameter, got {value:?}"),
            );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_auxiliary_parameter_propagates_a_default_error() {
        let mut state = CompileState::default();
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let dotted = Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span);
        let form = Form::list(vec![Form::atom("X", span), dotted], span);

        let error = state
            .compile_destructuring_auxiliary_parameter(&form, &mut seen)
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

#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_pattern(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructurePattern, CompileError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(DestructurePattern::Name(
                Self::compile_destructuring_binding_name(form, seen, "destructuring pattern name")?,
            )),
            FormKind::List(items) => Ok(DestructurePattern::List(
                items
                    .iter()
                    .map(|item| Self::compile_destructuring_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(DestructurePattern::Dotted {
                items: items
                    .iter()
                    .map(|item| Self::compile_destructuring_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(Self::compile_destructuring_pattern(tail, seen)?),
            }),
            _ => Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern must be a symbol or list".to_string(),
                },
                form.span,
            )),
        }
    }

    pub(super) fn compile_destructuring_binding_name(
        form: &Form,
        seen: &mut HashSet<String>,
        context: &str,
    ) -> Result<String, CompileError> {
        let name = Self::symbol_name(form, context)?;
        if name.starts_with('&') {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern does not support lambda-list markers"
                        .to_string(),
                },
                form.span,
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern names must be unique".to_string(),
                },
                form.span,
            ));
        }
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_destructuring_pattern_rejects_a_non_symbol_non_list_form() {
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = Form::new(FormKind::String("bad".to_string()), span);

        let error = CompileState::compile_destructuring_pattern(&form, &mut seen).map_or_else(
            |error| error,
            |value| panic!("a string literal cannot be a destructuring pattern, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_pattern_propagates_a_dotted_items_error() {
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let bad_item = Form::new(FormKind::String("bad".to_string()), span);
        let form = Form::dotted_list(vec![bad_item], Form::atom("REST", span), span);

        let error = CompileState::compile_destructuring_pattern(&form, &mut seen).map_or_else(
            |error| error,
            |value| panic!("a malformed dotted item should fail to compile, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_pattern_propagates_a_dotted_tail_error() {
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let bad_tail = Form::new(FormKind::String("bad".to_string()), span);
        let form = Form::dotted_list(vec![Form::atom("HEAD", span)], bad_tail, span);

        let error = CompileState::compile_destructuring_pattern(&form, &mut seen).map_or_else(
            |error| error,
            |value| panic!("a malformed dotted tail should fail to compile, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn compile_destructuring_binding_name_rejects_lambda_list_markers() {
        let mut seen = HashSet::new();
        let span = Span::new(0, 1);
        let form = Form::atom("&rest", span);

        let error = CompileState::compile_destructuring_binding_name(&form, &mut seen, "context")
            .map_or_else(
                |error| error,
                |value| panic!("a lambda-list marker cannot be a binding name, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::InvalidForm { message } if message.contains("lambda-list markers")
        ));
    }
}

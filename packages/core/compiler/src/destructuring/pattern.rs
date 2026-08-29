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

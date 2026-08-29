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

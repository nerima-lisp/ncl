use super::*;

impl CompileState {
    pub(super) fn compile_destructuring_binding_name(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        context: &str,
    ) -> Result<String, CompileError> {
        let name = self.symbol_name(form, context)?;
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

    pub(super) fn compile_destructuring_keyword_name(
        &self,
        form: &Form,
    ) -> Result<String, CompileError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "destructuring keyword name".to_string(),
                },
                form.span,
            ));
        };
        let Some(keyword) = name.strip_prefix(':') else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must start with a keyword"
                        .to_string(),
                },
                form.span,
            ));
        };
        if keyword.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must be nonempty".to_string(),
                },
                form.span,
            ));
        }
        Ok(normalize_name(keyword))
    }
}

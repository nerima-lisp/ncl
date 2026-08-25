use super::*;

impl CompileState {
    pub(super) fn symbol_name_info(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), CompileError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        };
        if token.kind != SymbolTokenKind::Symbol || token.name.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        }
        if token.escaped {
            if token.package.is_some() {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    },
                    form.span,
                ));
            }
            return Ok((token.name, true));
        }
        if literal_constant(name).is_some() || name.starts_with(':') {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            ));
        }
        Ok((normalize_name(name), false))
    }

    pub(super) fn symbol_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        self.symbol_name_info(form, context).map(|(name, _)| name)
    }

    pub(super) fn condition_name(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<String, CompileError> {
        Ok(self
            .control_name(form, context)?
            .trim_start_matches(':')
            .to_string())
    }

    pub(super) fn control_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        match &form.kind {
            FormKind::Atom(name)
                if !name.is_empty()
                    && ((name.starts_with(':') && name.len() > 1)
                        || (!name.starts_with(':')
                            && (literal_constant(name).is_none()
                                || name.eq_ignore_ascii_case("nil")
                                || name.eq_ignore_ascii_case("t")))) =>
            {
                Ok(normalize_name(name))
            }
            _ => Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            )),
        }
    }

    pub(super) fn control_tag(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        tag_name(form).ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            )
        })
    }
}

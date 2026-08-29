use crate::{
    CompileError, CompileErrorKind, CompileState, Form, FormKind, SymbolTokenKind,
    literal_constant, normalize_name, parse_symbol_token, tag_name,
};

impl CompileState {
    pub(crate) fn symbol_name_info(
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

    pub(crate) fn symbol_name(form: &Form, context: &str) -> Result<String, CompileError> {
        Self::symbol_name_info(form, context).map(|(name, _)| name)
    }

    pub(crate) fn condition_name(form: &Form, context: &str) -> Result<String, CompileError> {
        Ok(Self::control_name(form, context)?
            .trim_start_matches(':')
            .to_string())
    }

    pub(crate) fn control_name(form: &Form, context: &str) -> Result<String, CompileError> {
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

    pub(crate) fn control_tag(form: &Form, context: &str) -> Result<String, CompileError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    fn atom(source: &str) -> Form {
        Form::atom(source, Span::new(0, source.len()))
    }

    #[test]
    fn symbol_name_info_rejects_non_symbols_and_invalid_tokens() {
        let cases = [Form::list(Vec::new(), Span::new(0, 2)), atom("|")];

        for form in cases {
            assert!(CompileState::symbol_name_info(&form, "name").is_err());
        }
    }

    #[test]
    fn symbol_name_info_rejects_escaped_package_names() {
        let form = atom("pkg:|name|");

        assert!(CompileState::symbol_name_info(&form, "name").is_err());
    }
}

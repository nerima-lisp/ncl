#![allow(clippy::redundant_pub_crate)]

#[allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(super) fn symbol_name_info(
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

    pub(super) fn symbol_name(form: &Form, context: &str) -> Result<String, CompileError> {
        Self::symbol_name_info(form, context).map(|(name, _)| name)
    }

    pub(super) fn condition_name(form: &Form, context: &str) -> Result<String, CompileError> {
        Ok(Self::control_name(form, context)?
            .trim_start_matches(':')
            .to_string())
    }

    pub(super) fn control_name(form: &Form, context: &str) -> Result<String, CompileError> {
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

    pub(super) fn control_tag(form: &Form, context: &str) -> Result<String, CompileError> {
        tag_name(form).ok_or_else(|| {
            CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: context.to_string(),
                },
                form.span,
            )
        })
    }

    pub(super) fn require_arity(
        items: &[Form],
        operator: &str,
        expected: &str,
        expected_count: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if items.len().saturating_sub(1) != expected_count {
            return Err(Self::arity_error(items, operator, expected, span));
        }
        Ok(())
    }

    pub(super) fn arity_error(
        items: &[Form],
        operator: &str,
        expected: &str,
        span: Span,
    ) -> CompileError {
        CompileError::new(
            CompileErrorKind::Arity {
                operator: operator.to_string(),
                expected: expected.to_string(),
                actual: items.len().saturating_sub(1),
            },
            span,
        )
    }

    pub(super) fn internal_error(span: Span, message: &str) -> CompileError {
        CompileError::new(
            CompileErrorKind::Internal {
                message: message.to_string(),
            },
            span,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn internal_error_preserves_message_and_span() {
        let error = CompileState::internal_error(Span::new(4, 9), "invariant failed");

        assert_eq!(error.span, Span::new(4, 9));
        assert!(matches!(
            error.kind,
            CompileErrorKind::Internal { message } if message == "invariant failed"
        ));
    }

    #[test]
    fn compile_state_rejects_invalid_instruction_access() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());

        assert!(state.instruction_count(function + 1, span).is_err());
        assert!(state.emit(function + 1, Instruction::Return, span).is_err());
        assert!(state.patch_jump(function + 1, 0, 0, span).is_err());
        assert!(state.patch_jump(function, 0, 0, span).is_err());

        assert!(state.emit(function, Instruction::Return, span).is_ok());
        assert!(state.patch_jump(function, 0, 1, span).is_err());
    }

    #[test]
    fn compile_state_patches_only_jump_instructions() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());

        assert!(
            state
                .emit(function, Instruction::JumpIfFalse(0), span)
                .is_ok()
        );
        assert!(state.emit(function, Instruction::Jump(0), span).is_ok());
        assert!(state.patch_jump(function, 0, 7, span).is_ok());
        assert!(state.patch_jump(function, 1, 9, span).is_ok());
        assert!(state.patch_jump(function, 0, 0, span).is_ok());
    }

    #[test]
    fn compile_state_collects_names_and_skips_non_name_literals() {
        let span = Span::new(0, 1);
        let mut state = CompileState::default();
        let forms = vec![
            Form::list(
                vec![
                    atom("foo"),
                    Form::new(FormKind::Vector(vec![atom("bar")]), span),
                    Form::dotted_list(vec![atom("baz")], atom("tail"), span),
                ],
                span,
            ),
            Form::new(FormKind::String("ignored".to_string()), span),
            Form::new(FormKind::Character('x'), span),
        ];

        state.collect_names(&forms);
        assert!(state.used_names.contains("FOO"));
        assert!(state.used_names.contains("BAR"));
        assert!(state.used_names.contains("BAZ"));
        assert!(state.used_names.contains("TAIL"));
        assert_eq!(state.fresh_name("TEMP"), "__NCL_TEMP_0");
        state.used_names.insert("__NCL_TEMP_1".to_string());
        assert_eq!(state.fresh_name("TEMP"), "__NCL_TEMP_2");
    }
}

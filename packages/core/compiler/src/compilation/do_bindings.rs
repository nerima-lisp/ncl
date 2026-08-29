#![allow(clippy::wildcard_imports)]
use super::*;

type DoBinding = (String, bool, Option<Form>, Option<Form>);

/// The binding list's span, the termination form, its test/result forms,
/// and the per-variable `(name, escaped, init, step)` tuples parsed from a
/// `DO`/`DO*` form.
type ParsedDoForm<'a> = (Span, &'a Form, &'a [Form], Vec<DoBinding>);

impl CompileState {
    /// Parses and validates a `DO`/`DO*` form's bindings and termination
    /// clause, returning the pieces `compile_do` needs to emit code:
    /// the binding list's span, the termination form, its test/result
    /// forms, and the per-variable `(name, escaped, init, step)` tuples.
    pub(super) fn parse_do_form<'a>(
        items: &'a [Form],
        span: Span,
        operator: &str,
    ) -> Result<ParsedDoForm<'a>, CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, operator, "at least two", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing DO bindings after arity check",
            ));
        };
        let FormKind::List(binding_forms) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DO bindings".to_string(),
                },
                binding_form.span,
            ));
        };
        let Some(termination_form) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing DO termination after arity check",
            ));
        };
        let FormKind::List(termination) = &termination_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DO termination".to_string(),
                },
                termination_form.span,
            ));
        };
        if termination.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DO termination needs an end test".to_string(),
                },
                termination_form.span,
            ));
        }

        let mut names = HashSet::new();
        let mut parsed = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let FormKind::List(parts) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "DO binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if !(1..=3).contains(&parts.len()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "DO binding needs a name, optional init, and optional step"
                            .to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = parts.first() else {
                return Err(Self::internal_error(
                    binding.span,
                    "missing DO binding name",
                ));
            };
            let (name, escaped) = Self::symbol_name_info(name_form, "DO binding name")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "DO binding names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, parts.get(1).cloned(), parts.get(2).cloned()));
        }

        Ok((
            binding_form.span,
            termination_form,
            termination.as_slice(),
            parsed,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_do_form_rejects_a_non_list_binding_form() {
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("DO", span),
            Form::atom("BINDINGS", span),
            Form::list(vec![Form::atom("T", span)], span),
        ];

        let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
            |error| error,
            |value| panic!("bindings must be a list, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedList { context } if context == "DO bindings"
        ));
    }

    #[test]
    fn parse_do_form_rejects_a_binding_with_wrong_arity() {
        let span = Span::new(0, 1);
        let items = vec![
            Form::atom("DO", span),
            Form::list(vec![Form::list(Vec::new(), span)], span),
            Form::list(vec![Form::atom("T", span)], span),
        ];

        let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
            |error| error,
            |value| panic!("an empty binding has no name, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn parse_do_form_rejects_duplicate_binding_names() {
        let span = Span::new(0, 1);
        let bindings = Form::list(
            vec![
                Form::list(vec![Form::atom("X", span)], span),
                Form::list(vec![Form::atom("X", span)], span),
            ],
            span,
        );
        let items = vec![
            Form::atom("DO", span),
            bindings,
            Form::list(vec![Form::atom("T", span)], span),
        ];

        let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
            |error| error,
            |value| panic!("duplicate DO binding names must be rejected, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::InvalidForm { .. }));
    }

    #[test]
    fn parse_do_form_propagates_an_invalid_binding_name_error() {
        let span = Span::new(0, 1);
        let bindings = Form::list(vec![Form::list(vec![Form::atom(":x", span)], span)], span);
        let items = vec![
            Form::atom("DO", span),
            bindings,
            Form::list(vec![Form::atom("T", span)], span),
        ];

        let error = CompileState::parse_do_form(&items, span, "DO").map_or_else(
            |error| error,
            |value| panic!("a keyword cannot name a DO binding, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedSymbol { .. }
        ));
    }
}

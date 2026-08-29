#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_prog(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "PROG*" } else { "PROG" };
        if items.len() < 2 {
            return Err(Self::arity_error(items, operator, "at least one", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing PROG bindings after arity check",
            ));
        };
        let FormKind::List(binding_forms) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "PROG bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let parsed = Self::parse_prog_bindings(binding_forms)?;

        let prog_function = self.reserve_function(None, Vec::new());
        self.emit(prog_function, Instruction::EnterScope, binding_form.span)?;

        if sequential {
            self.compile_sequential_prog_bindings(prog_function, binding_form.span, &parsed)?;
        } else {
            self.compile_parallel_prog_bindings(prog_function, binding_form.span, &parsed)?;
        }

        self.compile_tagbody_forms(prog_function, span, items.get(2..).unwrap_or(&[]))?;
        self.emit(prog_function, Instruction::ExitScope, span)?;
        self.emit(prog_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: prog_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }

    fn parse_prog_bindings(
        binding_forms: &[Form],
    ) -> Result<Vec<(String, bool, Option<Form>)>, CompileError> {
        let mut names = HashSet::new();
        let mut parsed = Vec::with_capacity(binding_forms.len());
        for binding in binding_forms {
            let (name_form, init) = match &binding.kind {
                FormKind::Atom(_) => (binding, None),
                FormKind::List(parts) if (1..=2).contains(&parts.len()) => {
                    let Some(name_form) = parts.first() else {
                        return Err(Self::internal_error(
                            binding.span,
                            "missing PROG binding name",
                        ));
                    };
                    (name_form, parts.get(1).cloned())
                }
                FormKind::List(_) => {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "PROG binding needs a name and optional value".to_string(),
                        },
                        binding.span,
                    ));
                }
                _ => {
                    return Err(CompileError::new(
                        CompileErrorKind::ExpectedSymbol {
                            context: "PROG binding name".to_string(),
                        },
                        binding.span,
                    ));
                }
            };
            let (name, escaped) = Self::symbol_name_info(name_form, "PROG binding name")?;
            let key = if escaped {
                format!("\0{name}")
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "PROG binding names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, init));
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dotted(span: Span) -> Form {
        Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span)
    }

    #[test]
    fn compile_prog_propagates_a_sequential_binding_initializer_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let bindings = Form::list(
            vec![Form::list(vec![Form::atom("X", span), dotted(span)], span)],
            span,
        );
        let items = vec![Form::atom("PROG*", span), bindings];

        let error = state
            .compile_prog(function, span, &items, true)
            .map_or_else(
                |error| error,
                |value| {
                    panic!("a malformed binding initializer should fail to compile, got {value:?}")
                },
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_prog_propagates_a_parallel_binding_initializer_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let bindings = Form::list(
            vec![Form::list(vec![Form::atom("X", span), dotted(span)], span)],
            span,
        );
        let items = vec![Form::atom("PROG", span), bindings];

        let error = state
            .compile_prog(function, span, &items, false)
            .map_or_else(
                |error| error,
                |value| {
                    panic!("a malformed binding initializer should fail to compile, got {value:?}")
                },
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_prog_propagates_a_body_error() {
        let mut state = CompileState::default();
        let function = state.reserve_function(None, Vec::new());
        let span = Span::new(0, 1);
        let bindings = Form::list(Vec::new(), span);
        let items = vec![Form::atom("PROG", span), bindings, dotted(span)];

        let error = state
            .compile_prog(function, span, &items, false)
            .map_or_else(
                |error| error,
                |value| panic!("a malformed body form should fail to compile, got {value:?}"),
            );

        assert!(matches!(
            error.kind,
            CompileErrorKind::UnsupportedForm { .. }
        ));
    }

    #[test]
    fn compile_prog_reports_an_internal_error_for_an_invalid_function_id() {
        let mut state = CompileState::default();
        let span = Span::new(0, 1);
        let bindings = Form::list(Vec::new(), span);
        let items = vec![Form::atom("PROG", span), bindings];

        let error = state.compile_prog(99, span, &items, false).map_or_else(
            |error| error,
            |value| panic!("an unknown function id cannot receive instructions, got {value:?}"),
        );

        assert!(matches!(error.kind, CompileErrorKind::Internal { .. }));
    }

    #[test]
    fn parse_prog_bindings_rejects_a_non_symbol_non_list_binding() {
        let span = Span::new(0, 1);
        let bindings = [Form::new(FormKind::String("bad".to_string()), span)];

        let error = CompileState::parse_prog_bindings(&bindings).map_or_else(
            |error| error,
            |value| panic!("a literal cannot name a PROG binding, got {value:?}"),
        );

        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedSymbol { .. }
        ));
    }

    #[test]
    fn parse_prog_bindings_tracks_escaped_names_separately_from_normalized_ones() {
        let span = Span::new(0, 1);
        let bindings = [
            Form::list(vec![Form::atom("|X|", span)], span),
            Form::list(vec![Form::atom("x", span)], span),
        ];

        let parsed = CompileState::parse_prog_bindings(&bindings).unwrap_or_else(|error| {
            panic!("an escaped name and its normalized form do not collide: {error}")
        });

        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].1, "the first binding should be marked escaped");
        assert!(
            !parsed[1].1,
            "the second binding should not be marked escaped"
        );
    }
}

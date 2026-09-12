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
mod tests;

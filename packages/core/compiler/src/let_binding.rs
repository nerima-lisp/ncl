#[allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    #[expect(clippy::too_many_lines)]
    pub(super) fn compile_let(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: if sequential { "LET*" } else { "LET" }.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing let bindings after arity check",
            ));
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "let bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let (name_form, initializer) = if let FormKind::List(binding_items) = &binding.kind {
                if !(binding_items.len() == 1 || binding_items.len() == 2) {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidForm {
                            message: "let binding needs a name and optional value".to_string(),
                        },
                        binding.span,
                    ));
                }
                let Some(name_form) = binding_items.first() else {
                    return Err(Self::internal_error(
                        binding.span,
                        "missing let binding name",
                    ));
                };
                (name_form, binding_items.get(1))
            } else {
                (binding, None)
            };
            let (name, escaped) = Self::symbol_name_info(name_form, "let binding name")?;
            if !sequential && !names.insert(name.clone()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, initializer));
        }

        let body = items.get(2..).unwrap_or(&[]);
        let special_names = Self::special_declaration_names(body)?;
        let registered_special_names = self.special_names.clone();
        let is_special = |name: &str, escaped: bool| {
            special_names.contains(&(name.to_string(), escaped))
                || registered_special_names.contains(&(name.to_string(), escaped))
                || (!escaped && name.eq_ignore_ascii_case("*RANDOM-STATE*"))
        };
        if sequential {
            for (name, escaped, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                self.emit(function, Instruction::EnterScope, binding_form.span)?;
                if special_names
                    .iter()
                    .any(|(declared_name, declared_escaped)| {
                        declared_name == name && declared_escaped == escaped
                    })
                {
                    Self::emit_special_declarations(
                        self,
                        function,
                        &[(name.clone(), *escaped)],
                        binding_form.span,
                    )?;
                }
                let declared_special =
                    special_names
                        .iter()
                        .any(|(declared_name, declared_escaped)| {
                            declared_name == name && declared_escaped == escaped
                        });
                let define = if !declared_special && is_special(name, *escaped) {
                    if *escaped {
                        Instruction::DefineDynamicSpecialExact(name.clone())
                    } else {
                        Instruction::DefineDynamicSpecial(name.clone())
                    }
                } else if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(function, define, binding_form.span)?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
            if parsed.is_empty() {
                self.emit(function, Instruction::EnterScope, binding_form.span)?;
            }
        } else {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
            for (_, _, value) in &parsed {
                if let Some(value) = value {
                    self.compile_expression(function, value)?;
                } else {
                    self.emit(
                        function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
            }
            for (name, escaped, _) in parsed.iter().rev() {
                if special_names
                    .iter()
                    .any(|(declared_name, declared_escaped)| {
                        declared_name == name && declared_escaped == escaped
                    })
                {
                    Self::emit_special_declarations(
                        self,
                        function,
                        &[(name.clone(), *escaped)],
                        binding_form.span,
                    )?;
                }
                let declared_special =
                    special_names
                        .iter()
                        .any(|(declared_name, declared_escaped)| {
                            declared_name == name && declared_escaped == escaped
                        });
                let define = if !declared_special && is_special(name, *escaped) {
                    if *escaped {
                        Instruction::DefineDynamicSpecialExact(name.clone())
                    } else {
                        Instruction::DefineDynamicSpecial(name.clone())
                    }
                } else if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(function, define, binding_form.span)?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        self.compile_sequence(function, body)?;
        let scope_count = if sequential { parsed.len().max(1) } else { 1 };
        for _ in 0..scope_count {
            self.emit(function, Instruction::ExitScope, span)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

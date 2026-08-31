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

        let body = items.get(2..).unwrap_or(&[]);
        let mut special_names = HashSet::new();
        for form in body {
            let FormKind::List(declarations) = &form.kind else {
                break;
            };
            let Some(operator) = declarations.first() else {
                break;
            };
            if !matches!(&operator.kind, FormKind::Atom(name) if name.eq_ignore_ascii_case("DECLARE"))
            {
                break;
            }
            for declaration in declarations.iter().skip(1) {
                let FormKind::List(items) = &declaration.kind else {
                    continue;
                };
                let Some(operator) = items.first() else {
                    continue;
                };
                if !matches!(&operator.kind, FormKind::Atom(name) if name.eq_ignore_ascii_case("SPECIAL"))
                {
                    continue;
                }
                for name in items.iter().skip(1) {
                    if let Ok(name) = Self::symbol_name_info(name, "special declaration name") {
                        special_names.insert(name);
                    }
                }
            }
        }

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(binding_items) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "let binding".to_string(),
                    },
                    binding.span,
                ));
            };
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
            let (name, escaped) = Self::symbol_name_info(name_form, "let binding name")?;
            if !sequential && !names.insert(name.clone()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, escaped, binding_items.get(1)));
        }

        let registered_special_names = self.special_names.clone();
        let is_special = |name: &str, escaped: bool| {
            special_names.contains(&(name.to_string(), escaped))
                || registered_special_names.contains(&(name.to_string(), escaped))
                || (!escaped && name.eq_ignore_ascii_case("*RANDOM-STATE*"))
        };

        let binding_instruction = |name: &String, escaped: bool| {
            if special_names.contains(&(name.clone(), escaped)) {
                if escaped {
                    Instruction::DefineSpecialExact {
                        name: name.clone(),
                        force: true,
                    }
                } else {
                    Instruction::DefineSpecial {
                        name: name.clone(),
                        force: true,
                    }
                }
            } else if is_special(name, escaped) {
                Instruction::DefineDynamicSpecial(name.clone())
            } else if escaped {
                Instruction::DefineExact(name.clone())
            } else {
                Instruction::Define(name.clone())
            }
        };

        self.emit(function, Instruction::EnterScope, binding_form.span)?;
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
                self.emit(
                    function,
                    binding_instruction(name, *escaped),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        } else {
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
                self.emit(
                    function,
                    binding_instruction(name, *escaped),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

use super::*;

impl CompileState {
    pub(super) fn compile_dotimes(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DOTIMES", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DOTIMES binding after arity check"));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOTIMES binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOTIMES binding needs a variable, count, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(self.internal_error(spec_form.span, "missing DOTIMES variable"));
        };
        let variable = self.symbol_name(variable_form, "DOTIMES variable")?;
        let Some(count) = spec.get(1) else {
            return Err(self.internal_error(spec_form.span, "missing DOTIMES count"));
        };
        let result = spec.get(2);
        let limit = self.fresh_name("DOTIMES_LIMIT");

        self.emit(function, Instruction::EnterScope, spec_form.span)?;
        self.compile_expression(function, count)?;
        self.emit(function, Instruction::Define(limit.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(0)),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Define(variable.clone()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("<".to_string()),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Load(variable.clone()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(limit), spec_form.span)?;
        self.emit(function, Instruction::Call(2), spec_form.span)?;
        let exit_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            spec_form.span,
        )?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("+".to_string()),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Load(variable.clone()),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(1)),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Call(2), spec_form.span)?;
        self.emit(function, Instruction::Set(variable), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_form.span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        if let Some(result) = result {
            self.compile_expression(function, result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_dolist(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DOLIST", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DOLIST binding after arity check"));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOLIST binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOLIST binding needs a variable, list, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(self.internal_error(spec_form.span, "missing DOLIST variable"));
        };
        let variable = self.symbol_name(variable_form, "DOLIST variable")?;
        let Some(list) = spec.get(1) else {
            return Err(self.internal_error(spec_form.span, "missing DOLIST list"));
        };
        let result = spec.get(2);
        let tail = self.fresh_name("DOLIST_TAIL");

        self.emit(function, Instruction::EnterScope, spec_form.span)?;
        self.compile_expression(function, list)?;
        self.emit(function, Instruction::Define(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Define(variable.clone()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("ENDP".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        let body_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            spec_form.span,
        )?;
        let exit_jump = self.emit(function, Instruction::Jump(usize::MAX), spec_form.span)?;

        let body_start = self.instruction_count(function, span)?;
        self.patch_jump(function, body_jump, body_start, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CAR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        self.emit(function, Instruction::Set(variable.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CDR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        self.emit(function, Instruction::Set(tail), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_form.span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Set(variable), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        if let Some(result) = result {
            self.compile_expression(function, result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_do(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "DO*" } else { "DO" };
        if items.len() < 3 {
            return Err(self.arity_error(items, operator, "at least two", span));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DO bindings after arity check"));
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
            return Err(self.internal_error(span, "missing DO termination after arity check"));
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
                return Err(self.internal_error(binding.span, "missing DO binding name"));
            };
            let (name, escaped) = self.symbol_name_info(name_form, "DO binding name")?;
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

        let loop_function = self.reserve_function(None, Vec::new());
        self.emit(loop_function, Instruction::EnterScope, binding_form.span)?;

        if sequential {
            for (name, escaped, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_form.span)?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            let mut initial_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, init, _) in &parsed {
                if let Some(init) = init {
                    self.compile_expression(loop_function, init)?;
                } else {
                    self.emit(
                        loop_function,
                        Instruction::Constant(Constant::Nil),
                        binding_form.span,
                    )?;
                }
                let temporary = self.fresh_name("DO_INIT");
                self.emit(
                    loop_function,
                    Instruction::Define(temporary.clone()),
                    binding_form.span,
                )?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                initial_temporaries.push(temporary);
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(initial_temporaries) {
                self.emit(
                    loop_function,
                    Instruction::Load(temporary),
                    binding_form.span,
                )?;
                let define = if *escaped {
                    Instruction::DefineExact(name.clone())
                } else {
                    Instruction::Define(name.clone())
                };
                self.emit(loop_function, define, binding_form.span)?;
                self.emit(loop_function, Instruction::Pop, binding_form.span)?;
            }
        }

        let loop_start = self.instruction_count(loop_function, span)?;
        self.compile_expression(loop_function, &termination[0])?;
        let body_jump = self.emit(
            loop_function,
            Instruction::JumpIfFalse(usize::MAX),
            termination_form.span,
        )?;
        let result_jump = self.emit(
            loop_function,
            Instruction::Jump(usize::MAX),
            termination_form.span,
        )?;
        let body_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, body_jump, body_start, span)?;

        self.compile_tagbody_forms(loop_function, span, items.get(3..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::Pop, span)?;

        if sequential {
            for (name, escaped, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_form.span)?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                }
            }
        } else {
            let mut step_temporaries = Vec::with_capacity(parsed.len());
            for (_, _, _, step) in &parsed {
                if let Some(step) = step {
                    self.compile_expression(loop_function, step)?;
                    let temporary = self.fresh_name("DO_STEP");
                    self.emit(
                        loop_function,
                        Instruction::Define(temporary.clone()),
                        binding_form.span,
                    )?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                    step_temporaries.push(Some(temporary));
                } else {
                    step_temporaries.push(None);
                }
            }
            for ((name, escaped, _, _), temporary) in parsed.iter().zip(step_temporaries) {
                if let Some(temporary) = temporary {
                    self.emit(
                        loop_function,
                        Instruction::Load(temporary),
                        binding_form.span,
                    )?;
                    let set = if *escaped {
                        Instruction::SetExact(name.clone())
                    } else {
                        Instruction::Set(name.clone())
                    };
                    self.emit(loop_function, set, binding_form.span)?;
                    self.emit(loop_function, Instruction::Pop, binding_form.span)?;
                }
            }
        }
        self.emit(loop_function, Instruction::Jump(loop_start), span)?;

        let result_start = self.instruction_count(loop_function, span)?;
        self.patch_jump(loop_function, result_jump, result_start, span)?;
        self.compile_sequence(loop_function, termination.get(1..).unwrap_or(&[]))?;
        self.emit(loop_function, Instruction::ExitScope, span)?;
        self.emit(loop_function, Instruction::Return, span)?;
        self.emit(
            function,
            Instruction::Block {
                function: loop_function,
                name: "NIL".to_string(),
            },
            span,
        )?;
        Ok(())
    }
}

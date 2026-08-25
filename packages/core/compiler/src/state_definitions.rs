use super::*;

impl CompileState {
    pub(super) fn compile_define(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "DEFINE", "two", 2, span)?;
        let Some(name_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing define name after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing define value after arity check"));
        };
        let (name, escaped) = self.symbol_name_info(name_form, "define name")?;
        self.compile_expression(function, value_form)?;
        let instruction = if escaped {
            Instruction::DefineExact(name)
        } else {
            Instruction::Define(name)
        };
        self.emit(function, instruction, value_form.span)?;
        Ok(())
    }

    pub(super) fn compile_defun(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "defun needs a name, parameters, and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let Some(name_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing defun name after arity check"));
        };
        let Some(parameter_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing defun parameters after arity check"));
        };
        let (name, name_escaped) = self.symbol_name_info(name_form, "defun name")?;
        let lambda_list = self.parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            Some(name.clone()),
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.rest,
            lambda_list.rest_escaped,
        );
        let optional = self.compile_optional_parameters(&lambda_list.optional)?;
        self.functions[child].optional = optional;
        let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
        self.functions[child].keywords = keywords;
        self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
        self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
        let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
        self.functions[child].auxiliary = auxiliary;
        let body = items.get(3..).unwrap_or(&[]);
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;

        self.emit(function, Instruction::MakeClosure(child), span)?;
        let define = if name_escaped {
            Instruction::DefineExact(name.clone())
        } else {
            Instruction::Define(name.clone())
        };
        self.emit(function, define, span)?;
        self.emit(function, Instruction::Pop, span)?;
        let constant = if name_escaped {
            Constant::SymbolExact(name)
        } else {
            Constant::Symbol(name)
        };
        self.emit(function, Instruction::Constant(constant), span)?;
        Ok(())
    }

    pub(super) fn compile_setq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setq needs variable/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let pair_count = operands.len() / 2;
        for (index, pair) in operands.chunks_exact(2).enumerate() {
            let Some(name_form) = pair.first() else {
                return Err(self.internal_error(span, "missing setq target"));
            };
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing setq value"));
            };
            let (name, escaped) = self.symbol_name_info(name_form, "setq target")?;
            self.compile_expression(function, value_form)?;
            let instruction = if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            };
            self.emit(function, instruction, value_form.span)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }

    pub(super) fn compile_psetq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "psetq needs variable/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let mut names = Vec::with_capacity(operands.len() / 2);
        for pair in operands.chunks_exact(2) {
            let Some(name_form) = pair.first() else {
                return Err(self.internal_error(span, "missing psetq target"));
            };
            names.push(self.symbol_name_info(name_form, "psetq target")?);
        }
        for pair in operands.chunks_exact(2) {
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing psetq value"));
            };
            self.compile_expression(function, value_form)?;
        }
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::PsetqExact(names)
        } else {
            Instruction::Psetq(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    pub(super) fn compile_multiple_value_setq(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "MULTIPLE-VALUE-SETQ", "two", 2, span)?;
        let Some(variable_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-SETQ variables"));
        };
        let FormKind::List(variables) = &variable_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "MULTIPLE-VALUE-SETQ variables".to_string(),
                },
                variable_form.span,
            ));
        };
        let names = variables
            .iter()
            .map(|variable| self.symbol_name_info(variable, "MULTIPLE-VALUE-SETQ variable"))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing MULTIPLE-VALUE-SETQ value"));
        };
        self.compile_expression(function, value_form)?;
        let has_exact = names.iter().any(|(_, escaped)| *escaped);
        let instruction = if has_exact {
            Instruction::MultipleValueSetqExact(names)
        } else {
            Instruction::MultipleValueSetq(names.into_iter().map(|(name, _)| name).collect())
        };
        self.emit(function, instruction, value_form.span)?;
        Ok(())
    }

    pub(super) fn compile_setf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "setf needs place/value pairs".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let operands = items.get(1..).unwrap_or(&[]);
        let pair_count = operands.len() / 2;
        for (index, pair) in operands.chunks_exact(2).enumerate() {
            let Some(place) = pair.first() else {
                return Err(self.internal_error(span, "missing setf place"));
            };
            let Some(value_form) = pair.get(1) else {
                return Err(self.internal_error(span, "missing setf value"));
            };
            self.compile_expression(function, value_form)?;
            self.emit(function, Instruction::Setf(place.clone()), place.span)?;
            if index + 1 < pair_count {
                self.emit(function, Instruction::Pop, value_form.span)?;
            }
        }
        Ok(())
    }

    pub(super) fn compile_modify_symbol(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        operator: &str,
        arithmetic: &str,
    ) -> Result<(), CompileError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, operator, "one or two", span));
        }
        let place = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing modifying place"))?;
        let (name, escaped) = self.symbol_name_info(place, &format!("{operator} target"))?;
        self.emit(
            function,
            Instruction::FunctionLoad(arithmetic.to_string()),
            place.span,
        )?;
        self.compile_expression(function, place)?;
        if let Some(delta) = items.get(2) {
            self.compile_expression(function, delta)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Integer(1)), span)?;
        }
        self.emit(function, Instruction::Call(2), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::SetExact(name)
            } else {
                Instruction::Set(name)
            },
            place.span,
        )?;
        Ok(())
    }

    pub(super) fn compile_defvar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        force: bool,
    ) -> Result<(), CompileError> {
        let operator = if force { "DEFPARAMETER" } else { "DEFVAR" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity_error(items, operator, "one or two", span));
        }
        let name_form = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing defvar name"))?;
        let (name, escaped) = self.symbol_name_info(
            name_form,
            if force {
                "defparameter name"
            } else {
                "defvar name"
            },
        )?;
        if force {
            if let Some(initializer) = items.get(2) {
                self.compile_expression(function, initializer)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            self.emit(
                function,
                if escaped {
                    Instruction::DefineSpecialExact { name, force: true }
                } else {
                    Instruction::DefineSpecial { name, force: true }
                },
                span,
            )?;
            return Ok(());
        }

        self.emit(
            function,
            if escaped {
                Instruction::IsBoundExact(name.clone())
            } else {
                Instruction::IsBound(name.clone())
            },
            name_form.span,
        )?;
        let initialize_jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::LoadExact(name.clone())
            } else {
                Instruction::Load(name.clone())
            },
            name_form.span,
        )?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
        let initialize_target = self.instruction_count(function, span)?;
        if let Some(initializer) = items.get(2) {
            self.compile_expression(function, initializer)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            if escaped {
                Instruction::DefineSpecialExact { name, force: false }
            } else {
                Instruction::DefineSpecial { name, force: false }
            },
            span,
        )?;
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, initialize_jump, initialize_target, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
    }

    pub(super) fn compile_defstruct(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "DEFSTRUCT", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }

    pub(super) fn compile_runtime_definition(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(self.arity_error(items, "runtime definition", "at least one", span));
        }
        self.emit(
            function,
            Instruction::Quote(Form::list(items.to_vec(), span)),
            span,
        )?;
        self.emit(function, Instruction::Eval(span), span)?;
        Ok(())
    }
}

use super::*;

impl CompileState {
    pub(super) fn compile_lambda(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "lambda needs parameters and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let Some(parameter_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing lambda parameters after arity check"));
        };
        let lambda_list = self.parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            None,
            lambda_list.required.clone(),
            lambda_list.required_escaped.clone(),
            lambda_list.rest.clone(),
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
        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::MakeClosure(child), span)?;
        Ok(())
    }

    pub(super) fn compile_function(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        self.require_arity(items, "FUNCTION", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(self.internal_error(span, "missing function argument after arity check"));
        };
        if matches!(argument.kind, FormKind::Atom(_)) {
            let (name, escaped) = self.symbol_name_info(argument, "function name")?;
            let local_function = self.is_local_function(&Self::local_function_key(&name, escaped));
            self.emit(
                function,
                if local_function && escaped {
                    Instruction::FunctionLoadExact(name)
                } else if local_function {
                    Instruction::FunctionLoad(name)
                } else if escaped {
                    Instruction::FunctionLoadExact(name)
                } else {
                    Instruction::FunctionLoad(name)
                },
                argument.span,
            )?;
        } else {
            self.compile_expression(function, argument)?;
        }
        Ok(())
    }

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
        let documentation = match &items[3].kind {
            FormKind::String(value) => Some(value.clone()),
            _ => None,
        };
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
        if let Some(documentation) = documentation {
            self.emit(
                function,
                Instruction::DefineFunctionDocumentation {
                    name: name.clone(),
                    exact: name_escaped,
                    documentation,
                },
                span,
            )?;
        }
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

    pub(super) fn compile_push(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(2)
            .ok_or_else(|| self.internal_error(span, "missing PUSH place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "push target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        self.compile_expression(function, &items[1])?;
        let instruction = if escaped {
            Instruction::PushExact(name)
        } else {
            Instruction::Push(name)
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    pub(super) fn compile_pop(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 2 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(1)
            .ok_or_else(|| self.internal_error(span, "missing POP place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "pop target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        let instruction = if escaped {
            Instruction::PopPlaceExact(name)
        } else {
            Instruction::PopPlace(name)
        };
        self.emit(function, instruction, span)?;
        Ok(())
    }

    pub(super) fn compile_pushnew(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return self.compile_runtime_definition(function, span, items);
        }
        let place = items
            .get(2)
            .ok_or_else(|| self.internal_error(span, "missing PUSHNEW place"))?;
        if !matches!(&place.kind, FormKind::Atom(_)) {
            return self.compile_runtime_definition(function, span, items);
        }
        let Ok((name, escaped)) = self.symbol_name_info(place, "pushnew target") else {
            return self.compile_runtime_definition(function, span, items);
        };
        self.compile_expression(function, &items[1])?;
        let instruction = if escaped {
            Instruction::PushNewExact(name)
        } else {
            Instruction::PushNew(name)
        };
        self.emit(function, instruction, span)?;
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
        if !(2..=4).contains(&items.len()) {
            return Err(self.arity_error(items, operator, "one to three", span));
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
        let documentation = match items.get(3) {
            Some(Form {
                kind: FormKind::String(documentation),
                ..
            }) => Some(documentation.clone()),
            Some(form) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: format!("{} documentation must be a string", operator),
                    },
                    form.span,
                ));
            }
            None => None,
        };
        if force {
            if let Some(initializer) = items.get(2) {
                self.compile_expression(function, initializer)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            self.emit(
                function,
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
                },
                span,
            )?;
            if let Some(documentation) = documentation {
                self.emit(
                    function,
                    Instruction::DefineVariableDocumentation {
                        name,
                        exact: escaped,
                        documentation,
                    },
                    span,
                )?;
            }
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
                Instruction::DefineSpecialExact {
                    name: name.clone(),
                    force: false,
                }
            } else {
                Instruction::DefineSpecial {
                    name: name.clone(),
                    force: false,
                }
            },
            span,
        )?;
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, initialize_jump, initialize_target, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        if let Some(documentation) = documentation {
            self.emit(
                function,
                Instruction::DefineVariableDocumentation {
                    name,
                    exact: escaped,
                    documentation,
                },
                span,
            )?;
        }
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

    pub(super) fn compile_defconstant(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(items.len() == 3 || items.len() == 4) {
            return Err(self.arity_error(items, "DEFCONSTANT", "two or three", span));
        }
        let Some(name_form) = items.get(1) else {
            return Err(self.internal_error(span, "missing DEFCONSTANT name after arity check"));
        };
        let Some(value_form) = items.get(2) else {
            return Err(self.internal_error(span, "missing DEFCONSTANT value after arity check"));
        };
        let (name, escaped) = self.symbol_name_info(name_form, "DEFCONSTANT name")?;
        self.emit(
            function,
            if escaped {
                Instruction::CheckConstantExact(name.clone())
            } else {
                Instruction::CheckConstant(name.clone())
            },
            name_form.span,
        )?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            if escaped {
                Instruction::DefineConstantExact(name)
            } else {
                Instruction::DefineConstant(name)
            },
            span,
        )?;
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

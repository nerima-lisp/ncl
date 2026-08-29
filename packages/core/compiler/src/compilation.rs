#![allow(clippy::wildcard_imports)]
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
        let parameter_form = &items[1];
        let lambda_list = Self::parameters(parameter_form)?;
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
        Self::require_arity(items, "FUNCTION", "one", 1, span)?;
        let argument = &items[1];
        if matches!(argument.kind, FormKind::Atom(_)) {
            let (name, escaped) = Self::symbol_name_info(argument, "function name")?;
            self.emit(
                function,
                if escaped {
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
        Self::require_arity(items, "DEFINE", "two", 2, span)?;
        let name_form = &items[1];
        let value_form = &items[2];
        let (name, escaped) = Self::symbol_name_info(name_form, "define name")?;
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
        let name_form = &items[1];
        let parameter_form = &items[2];
        let (name, name_escaped) = Self::symbol_name_info(name_form, "defun name")?;
        let lambda_list = Self::parameters(parameter_form)?;
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
        let (pairs, _) = operands.as_chunks::<2>();
        let pair_count = operands.len() / 2;
        for (index, [name_form, value_form]) in pairs.iter().enumerate() {
            let (name, escaped) = Self::symbol_name_info(name_form, "setq target")?;
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
        let (pairs, _) = operands.as_chunks::<2>();
        let mut names = Vec::with_capacity(operands.len() / 2);
        for [name_form, _] in pairs {
            names.push(Self::symbol_name_info(name_form, "psetq target")?);
        }
        for [_, value_form] in pairs {
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
        Self::require_arity(items, "MULTIPLE-VALUE-SETQ", "two", 2, span)?;
        let Some(variable_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-SETQ variables",
            ));
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
            .map(|variable| Self::symbol_name_info(variable, "MULTIPLE-VALUE-SETQ variable"))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(value_form) = items.get(2) else {
            return Err(Self::internal_error(
                span,
                "missing MULTIPLE-VALUE-SETQ value",
            ));
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
        let (pairs, _) = operands.as_chunks::<2>();
        let pair_count = operands.len() / 2;
        for (index, [place, value_form]) in pairs.iter().enumerate() {
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
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let place = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing modifying place"))?;
        let (name, escaped) = Self::symbol_name_info(place, &format!("{operator} target"))?;
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
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let name_form = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing defvar name"))?;
        let (name, escaped) = Self::symbol_name_info(
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

    pub(super) fn compile_funcall(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "FUNCALL", "at least one", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_eval(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "EVAL", "one", 1, span)?;
        let Some(argument) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing eval argument after arity check",
            ));
        };
        self.compile_expression(function, argument)?;
        self.emit(function, Instruction::Eval(argument.span), span)?;
        Ok(())
    }

    pub(super) fn compile_apply(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "APPLY", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Apply(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_mapcar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAPCAR", "at least two", span));
        }
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::MapCar(items.len().saturating_sub(2)),
            span,
        )?;
        Ok(())
    }

    pub(super) fn compile_map_into(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(Self::arity_error(items, "MAP-INTO", "at least two", span));
        }
        let destination = items[1].clone();
        self.emit(
            function,
            Instruction::FunctionLoad("MAP-INTO".to_string()),
            items[0].span,
        )?;
        for item in &items[1..] {
            self.compile_expression(function, item)?;
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        self.emit(
            function,
            Instruction::MapIntoSetf(destination.clone()),
            destination.span,
        )?;
        Ok(())
    }

    pub(super) fn compile_dotimes(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DOTIMES", "at least one", span));
        }
        let (spec_span, variable, count, result) = Self::parse_dotimes_spec(items, span)?;
        let limit = self.fresh_name("DOTIMES_LIMIT");

        self.emit(function, Instruction::EnterScope, spec_span)?;
        self.compile_expression(function, &count)?;
        self.emit(function, Instruction::Define(limit.clone()), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(0)),
            spec_span,
        )?;
        self.emit(function, Instruction::Define(variable.clone()), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("<".to_string()),
            spec_span,
        )?;
        self.emit(function, Instruction::Load(variable.clone()), spec_span)?;
        self.emit(function, Instruction::Load(limit), spec_span)?;
        self.emit(function, Instruction::Call(2), spec_span)?;
        let exit_jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), spec_span)?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("+".to_string()),
            spec_span,
        )?;
        self.emit(function, Instruction::Load(variable.clone()), spec_span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(1)),
            spec_span,
        )?;
        self.emit(function, Instruction::Call(2), spec_span)?;
        self.emit(function, Instruction::Set(variable), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        if let Some(result) = result {
            self.compile_expression(function, &result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn parse_dotimes_spec(
        items: &[Form],
        span: Span,
    ) -> Result<(Span, String, Form, Option<Form>), CompileError> {
        let Some(spec_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing DOTIMES binding after arity check",
            ));
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
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOTIMES variable",
            ));
        };
        let variable = Self::symbol_name(variable_form, "DOTIMES variable")?;
        let Some(count) = spec.get(1) else {
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOTIMES count",
            ));
        };
        Ok((
            spec_form.span,
            variable,
            count.clone(),
            spec.get(2).cloned(),
        ))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn compile_dolist(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DOLIST", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing DOLIST binding after arity check",
            ));
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
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOLIST variable",
            ));
        };
        let variable = Self::symbol_name(variable_form, "DOLIST variable")?;
        let Some(list) = spec.get(1) else {
            return Err(Self::internal_error(spec_form.span, "missing DOLIST list"));
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

    #[allow(clippy::too_many_lines)]
    pub(super) fn compile_do(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        sequential: bool,
    ) -> Result<(), CompileError> {
        let operator = if sequential { "DO*" } else { "DO" };
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

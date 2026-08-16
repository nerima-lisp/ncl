use super::*;

impl CompileState {
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
            return Err(self.internal_error(span, "missing let bindings after arity check"));
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
                return Err(self.internal_error(binding.span, "missing let binding name"));
            };
            let name = self.symbol_name(name_form, "let binding name")?;
            if !sequential && !names.insert(name.clone()) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "let bindings must have distinct names".to_string(),
                    },
                    name_form.span,
                ));
            }
            parsed.push((name, binding_items.get(1)));
        }

        self.emit(function, Instruction::EnterScope, binding_form.span)?;
        if sequential {
            for (name, value) in &parsed {
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
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        } else {
            for (_, value) in &parsed {
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
            for (name, _) in parsed.iter().rev() {
                self.emit(
                    function,
                    Instruction::Define(name.clone()),
                    binding_form.span,
                )?;
                self.emit(function, Instruction::Pop, binding_form.span)?;
            }
        }

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn compile_flet(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        recursive: bool,
    ) -> Result<(), CompileError> {
        let operator = if recursive { "LABELS" } else { "FLET" };
        if items.len() < 2 {
            return Err(CompileError::new(
                CompileErrorKind::Arity {
                    operator: operator.to_string(),
                    expected: "at least one".to_string(),
                    actual: items.len().saturating_sub(1),
                },
                operator_span(items, span),
            ));
        }
        let Some(binding_form) = items.get(1) else {
            return Err(
                self.internal_error(span, "missing local function bindings after arity check")
            );
        };
        let FormKind::List(bindings) = &binding_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "local function bindings".to_string(),
                },
                binding_form.span,
            ));
        };

        let mut parsed = Vec::with_capacity(bindings.len());
        let mut names = HashSet::new();
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(CompileError::new(
                    CompileErrorKind::ExpectedList {
                        context: "local function binding".to_string(),
                    },
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function needs a name, parameters, and a body".to_string(),
                    },
                    binding.span,
                ));
            }
            let Some(name_form) = parts.first() else {
                return Err(self.internal_error(
                    binding.span,
                    "missing local function name after arity check",
                ));
            };
            let (name, name_escaped) = self.symbol_name_info(name_form, "local function name")?;
            let local_key = Self::local_function_key(&name, name_escaped);
            if !names.insert(local_key) {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "local function names must be unique".to_string(),
                    },
                    name_form.span,
                ));
            }
            let Some(parameter_form) = parts.get(1) else {
                return Err(self.internal_error(
                    binding.span,
                    "missing local function parameters after arity check",
                ));
            };
            let lambda_list = self.parameters(parameter_form)?;
            parsed.push((name, name_escaped, lambda_list, parts[2..].to_vec()));
        }

        if recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
            self.local_function_scopes.push(names.clone());
        }
        for (name, _name_escaped, lambda_list, body) in &parsed {
            let child = self.reserve_function_with_rest(
                Some(name.clone()),
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
            self.compile_sequence(child, body)?;
            self.emit(child, Instruction::Return, span)?;
            self.emit(function, Instruction::MakeClosure(child), span)?;
        }
        if !recursive {
            self.emit(function, Instruction::EnterScope, binding_form.span)?;
        }
        for (name, name_escaped, _, _) in parsed.iter().rev() {
            let instruction = if *name_escaped {
                Instruction::DefineFunctionExact(name.clone())
            } else {
                Instruction::DefineFunction(name.clone())
            };
            self.emit(function, instruction, binding_form.span)?;
        }

        let body = items.get(2..).unwrap_or(&[]);
        if !recursive {
            self.local_function_scopes.push(names);
        }
        self.compile_sequence(function, body)?;
        self.local_function_scopes.pop();
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    pub(super) fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, CompileError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let span = error.span;
            let kind = match error.kind {
                LambdaListErrorKind::ExpectedList => CompileErrorKind::ExpectedList {
                    context: "parameters".to_string(),
                },
                LambdaListErrorKind::ExpectedSymbol { context } => {
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    }
                }
                LambdaListErrorKind::InvalidForm { message } => {
                    CompileErrorKind::InvalidForm { message }
                }
            };
            CompileError::new(kind, span)
        })
    }

    pub(super) fn compile_optional_parameters(
        &mut self,
        specifications: &[LambdaListOptionalParameter],
    ) -> Result<Vec<OptionalParameter>, CompileError> {
        let mut optional = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            optional.push(OptionalParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(optional)
    }

    pub(super) fn compile_auxiliary_parameters(
        &mut self,
        specifications: &[LambdaListAuxiliaryParameter],
    ) -> Result<Vec<AuxiliaryParameter>, CompileError> {
        let mut auxiliary = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            auxiliary.push(AuxiliaryParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
            });
        }
        Ok(auxiliary)
    }

    pub(super) fn compile_keyword_parameters(
        &mut self,
        specifications: &[LambdaListKeywordParameter],
    ) -> Result<Vec<KeywordParameter>, CompileError> {
        let mut keywords = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            keywords.push(KeywordParameter {
                keyword_name: specification.keyword_name.clone(),
                keyword_name_escaped: specification.keyword_name_escaped,
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(keywords)
    }

    pub(super) fn symbol_name_info(
        &self,
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

    pub(super) fn symbol_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
        self.symbol_name_info(form, context).map(|(name, _)| name)
    }

    pub(super) fn condition_name(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<String, CompileError> {
        Ok(self
            .control_name(form, context)?
            .trim_start_matches(':')
            .to_string())
    }

    pub(super) fn control_name(&self, form: &Form, context: &str) -> Result<String, CompileError> {
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

    pub(super) fn control_tag(&self, form: &Form, context: &str) -> Result<String, CompileError> {
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
        &self,
        items: &[Form],
        operator: &str,
        expected: &str,
        expected_count: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if items.len().saturating_sub(1) != expected_count {
            return Err(self.arity_error(items, operator, expected, span));
        }
        Ok(())
    }

    pub(super) fn arity_error(
        &self,
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

    pub(super) fn internal_error(&self, span: Span, message: &str) -> CompileError {
        CompileError::new(
            CompileErrorKind::Internal {
                message: message.to_string(),
            },
            span,
        )
    }
}

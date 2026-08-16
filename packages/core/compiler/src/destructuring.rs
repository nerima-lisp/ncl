use super::*;

impl CompileState {
    pub(super) fn compile_destructuring_pattern(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructurePattern, CompileError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(DestructurePattern::Name(
                self.compile_destructuring_binding_name(form, seen, "destructuring pattern name")?,
            )),
            FormKind::List(items) => {
                if items.iter().any(|item| {
                    matches!(
                        &item.kind,
                        FormKind::Atom(name) if normalize_name(name).starts_with('&')
                    )
                }) {
                    Ok(DestructurePattern::LambdaList(
                        self.compile_destructuring_lambda_list_with_seen(form, seen)?,
                    ))
                } else {
                    Ok(DestructurePattern::List(
                        items
                            .iter()
                            .map(|item| self.compile_destructuring_pattern(item, seen))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                }
            }
            FormKind::DottedList { items, tail } => Ok(DestructurePattern::Dotted {
                items: items
                    .iter()
                    .map(|item| self.compile_destructuring_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.compile_destructuring_pattern(tail, seen)?),
            }),
            _ => Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern must be a symbol or list".to_string(),
                },
                form.span,
            )),
        }
    }

    pub(super) fn compile_destructuring_binding_name(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        context: &str,
    ) -> Result<String, CompileError> {
        let name = self.symbol_name(form, context)?;
        if name.starts_with('&') {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern does not support lambda-list markers"
                        .to_string(),
                },
                form.span,
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring pattern names must be unique".to_string(),
                },
                form.span,
            ));
        }
        Ok(name)
    }

    pub(super) fn compile_destructuring_default(
        &mut self,
        form: &Form,
    ) -> Result<FunctionId, CompileError> {
        let default_function = self.reserve_function(None, Vec::new());
        self.compile_expression(default_function, form)?;
        self.emit(default_function, Instruction::Return, form.span)?;
        Ok(default_function)
    }

    pub(super) fn compile_destructuring_optional_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureOptionalParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (pattern, init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (self.compile_destructuring_pattern(form, seen)?, nil(), None),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = self.compile_destructuring_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| {
                        self.compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?;
                (pattern, init_form, supplied_p)
            }
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must contain one to three items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring optional parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureOptionalParameter {
            pattern,
            default_function,
            supplied_p,
        })
    }

    pub(super) fn compile_destructuring_keyword_name(
        &self,
        form: &Form,
    ) -> Result<String, CompileError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "destructuring keyword name".to_string(),
                },
                form.span,
            ));
        };
        let Some(keyword) = name.strip_prefix(':') else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must start with a keyword"
                        .to_string(),
                },
                form.span,
            ));
        };
        if keyword.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must be nonempty".to_string(),
                },
                form.span,
            ));
        }
        Ok(normalize_name(keyword))
    }

    pub(super) fn compile_destructuring_keyword_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureKeywordParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = self.compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring keyword parameter name",
                )?;
                let keyword_name = normalize_name(&name);
                (keyword_name, DestructurePattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword designator must contain a keyword and variable"
                                    .to_string(),
                            },
                            items[0].span,
                        ));
                    }
                    let keyword_name =
                        self.compile_destructuring_keyword_name(&key_specification[0])?;
                    let pattern =
                        self.compile_destructuring_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if matches!(&items[0].kind, FormKind::Atom(name) if name.starts_with(':')) {
                    let keyword_name = self.compile_destructuring_keyword_name(&items[0])?;
                    if items.len() < 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword parameter needs a variable"
                                    .to_string(),
                            },
                            form.span,
                        ));
                    }
                    let pattern = self.compile_destructuring_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = self.compile_destructuring_pattern(&items[0], seen)?;
                    let DestructurePattern::Name(name) = &pattern else {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message:
                                    "destructuring keyword parameter must have a variable name"
                                        .to_string(),
                            },
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring keyword parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };

        let item_count = match &form.kind {
            FormKind::Atom(_) => 0,
            FormKind::List(items) => items.len(),
            _ => unreachable!(),
        };
        if item_count > trailing_start + 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword parameter contains too many items".to_string(),
                },
                form.span,
            ));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| {
                        self.compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureKeywordParameter {
            keyword_name,
            pattern,
            default_function,
            supplied_p,
        })
    }

    pub(super) fn compile_destructuring_auxiliary_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureAuxiliaryParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (name, init_form) = match &form.kind {
            FormKind::Atom(_) => (
                self.compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                nil(),
            ),
            FormKind::List(items) if (1..=2).contains(&items.len()) => (
                self.compile_destructuring_binding_name(
                    &items[0],
                    seen,
                    "destructuring auxiliary parameter name",
                )?,
                items.get(1).cloned().unwrap_or_else(nil),
            ),
            FormKind::List(_) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must contain one or two items"
                            .to_string(),
                    },
                    form.span,
                ));
            }
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring auxiliary parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureAuxiliaryParameter {
            name,
            default_function,
        })
    }

    pub(super) fn compile_destructuring_lambda_list(
        &mut self,
        form: &Form,
    ) -> Result<DestructureLambdaList, CompileError> {
        let mut seen = HashSet::new();
        self.compile_destructuring_lambda_list_with_seen(form, &mut seen)
    }

    pub(super) fn compile_destructuring_lambda_list_with_seen(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureLambdaList, CompileError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "destructuring lambda list".to_string(),
                },
                form.span,
            ));
        };
        let mut lambda_list = DestructureLambdaList {
            whole: None,
            environment: None,
            required: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            auxiliary: Vec::new(),
        };
        let mut section = DestructureLambdaListSection::Required;
        let mut index = 0;
        while index < parameters.len() {
            let parameter = &parameters[index];
            if let FormKind::Atom(name) = &parameter.kind {
                let marker = normalize_name(name);
                match marker.as_str() {
                    "&WHOLE" => {
                        if index != 0
                            || lambda_list.whole.is_some()
                            || index + 1 >= parameters.len()
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message:
                                        "&whole must be the first marker followed by one parameter"
                                            .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.whole = Some(self.compile_destructuring_binding_name(
                            &parameters[index + 1],
                            seen,
                            "destructuring whole parameter name",
                        )?);
                        index += 2;
                    }
                    "&OPTIONAL" => {
                        if section != DestructureLambdaListSection::Required {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message:
                                        "&optional is out of order in destructuring lambda list"
                                            .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        section = DestructureLambdaListSection::Optional;
                        index += 1;
                    }
                    "&REST" | "&BODY" => {
                        if lambda_list.rest.is_some()
                            || matches!(
                                section,
                                DestructureLambdaListSection::Rest
                                    | DestructureLambdaListSection::Keyword
                                    | DestructureLambdaListSection::Auxiliary
                            )
                            || index + 1 >= parameters.len()
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&rest or &body must be followed by one parameter"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.rest = Some(self.compile_destructuring_binding_name(
                            &parameters[index + 1],
                            seen,
                            "destructuring rest parameter name",
                        )?);
                        section = DestructureLambdaListSection::Rest;
                        index += 2;
                    }
                    "&KEY" => {
                        if lambda_list.has_keyword_section
                            || matches!(
                                section,
                                DestructureLambdaListSection::Keyword
                                    | DestructureLambdaListSection::Auxiliary
                            )
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&key is out of order or repeated in destructuring lambda list"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.has_keyword_section = true;
                        section = DestructureLambdaListSection::Keyword;
                        index += 1;
                    }
                    "&ALLOW-OTHER-KEYS" => {
                        if section != DestructureLambdaListSection::Keyword
                            || lambda_list.allow_other_keys
                        {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&allow-other-keys requires a keyword section"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.allow_other_keys = true;
                        index += 1;
                    }
                    "&AUX" => {
                        if section == DestructureLambdaListSection::Auxiliary {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&aux is repeated in destructuring lambda list"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        section = DestructureLambdaListSection::Auxiliary;
                        index += 1;
                    }
                    "&ENVIRONMENT" => {
                        if lambda_list.environment.is_some() || index + 1 >= parameters.len() {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "&environment must be followed by one parameter"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        lambda_list.environment = Some(self.compile_destructuring_binding_name(
                            &parameters[index + 1],
                            seen,
                            "destructuring environment parameter name",
                        )?);
                        index += 2;
                    }
                    _ if marker.starts_with('&') => {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "unsupported marker in destructuring lambda list"
                                    .to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    _ => {
                        if section == DestructureLambdaListSection::Rest {
                            return Err(CompileError::new(
                                CompileErrorKind::InvalidForm {
                                    message: "destructuring rest parameter must be followed by a keyword or auxiliary section"
                                        .to_string(),
                                },
                                parameter.span,
                            ));
                        }
                        match section {
                            DestructureLambdaListSection::Required => lambda_list
                                .required
                                .push(self.compile_destructuring_pattern(parameter, seen)?),
                            DestructureLambdaListSection::Optional => lambda_list.optional.push(
                                self.compile_destructuring_optional_parameter(parameter, seen)?,
                            ),
                            DestructureLambdaListSection::Keyword => {
                                if lambda_list.allow_other_keys {
                                    return Err(CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: "&allow-other-keys must be the last keyword-list marker"
                                                .to_string(),
                                        },
                                        parameter.span,
                                    ));
                                }
                                let specification =
                                    self.compile_destructuring_keyword_parameter(parameter, seen)?;
                                if lambda_list
                                    .keywords
                                    .iter()
                                    .any(|item| item.keyword_name == specification.keyword_name)
                                {
                                    return Err(CompileError::new(
                                        CompileErrorKind::InvalidForm {
                                            message: "destructuring keyword names must be unique"
                                                .to_string(),
                                        },
                                        parameter.span,
                                    ));
                                }
                                lambda_list.keywords.push(specification);
                            }
                            DestructureLambdaListSection::Auxiliary => lambda_list.auxiliary.push(
                                self.compile_destructuring_auxiliary_parameter(parameter, seen)?,
                            ),
                            DestructureLambdaListSection::Rest => unreachable!(),
                        }
                        index += 1;
                    }
                }
                continue;
            }

            if section == DestructureLambdaListSection::Rest {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring rest parameter must be followed by a keyword or auxiliary section"
                            .to_string(),
                    },
                    parameter.span,
                ));
            }
            match section {
                DestructureLambdaListSection::Required => lambda_list
                    .required
                    .push(self.compile_destructuring_pattern(parameter, seen)?),
                DestructureLambdaListSection::Optional => lambda_list
                    .optional
                    .push(self.compile_destructuring_optional_parameter(parameter, seen)?),
                DestructureLambdaListSection::Keyword => {
                    if lambda_list.allow_other_keys {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "&allow-other-keys must be the last keyword-list marker"
                                    .to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    let specification =
                        self.compile_destructuring_keyword_parameter(parameter, seen)?;
                    if lambda_list
                        .keywords
                        .iter()
                        .any(|item| item.keyword_name == specification.keyword_name)
                    {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword names must be unique".to_string(),
                            },
                            parameter.span,
                        ));
                    }
                    lambda_list.keywords.push(specification);
                }
                DestructureLambdaListSection::Auxiliary => lambda_list
                    .auxiliary
                    .push(self.compile_destructuring_auxiliary_parameter(parameter, seen)?),
                DestructureLambdaListSection::Rest => unreachable!(),
            }
            index += 1;
        }

        Ok(lambda_list)
    }

    pub(super) fn compile_destructuring_bind(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(self.arity_error(items, "DESTRUCTURING-BIND", "two or more", span));
        }
        let mut seen = HashSet::new();
        let specification = match &items[1].kind {
            FormKind::List(_) => {
                DestructureSpec::LambdaList(self.compile_destructuring_lambda_list(&items[1])?)
            }
            _ => {
                DestructureSpec::Pattern(self.compile_destructuring_pattern(&items[1], &mut seen)?)
            }
        };
        self.emit(function, Instruction::EnterScope, items[1].span)?;
        self.compile_expression(function, &items[2])?;
        self.emit(
            function,
            Instruction::Destructure(specification),
            items[1].span,
        )?;
        self.compile_sequence(function, items.get(3..).unwrap_or(&[]))?;
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}

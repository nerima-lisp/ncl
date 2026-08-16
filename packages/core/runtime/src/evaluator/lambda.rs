macro_rules! evaluator_lambda {
    () => {
    fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            self.invalid(&message, error.span)
        })
    }

    fn macro_parameters(
        &self,
        form: &Form,
        destructuring: bool,
    ) -> Result<MacroLambdaList, RuntimeError> {
        let mut seen = HashSet::new();
        self.macro_parameters_with_seen(form, destructuring, &mut seen)
    }

    fn macro_parameters_with_seen(
        &self,
        form: &Form,
        destructuring: bool,
        seen: &mut HashSet<String>,
    ) -> Result<MacroLambdaList, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(self.invalid("macro parameters must be a list", form.span));
        };

        let mut lambda_list = MacroLambdaList {
            whole: None,
            environment: None,
            required: Vec::new(),
            optional: Vec::new(),
            rest: None,
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            auxiliary: Vec::new(),
        };
        let mut section = MacroLambdaListSection::Required;
        let mut index = 0;

        while index < parameters.len() {
            let parameter = &parameters[index];
            if let Some(name) = atom_name(parameter) {
                let marker = normalize_name(name);
                match marker.as_str() {
                    "&WHOLE" => {
                        if index != 0
                            || lambda_list.whole.is_some()
                            || index + 1 >= parameters.len()
                        {
                            return Err(self.invalid(
                                "&whole must be the first marker followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.whole = Some(self.macro_binding_name(
                            &parameters[index + 1],
                            seen,
                            destructuring,
                        )?);
                        index += 2;
                    }
                    "&OPTIONAL" => {
                        if section != MacroLambdaListSection::Required {
                            return Err(self.invalid(
                                "&optional is out of order in macro lambda list",
                                parameter.span,
                            ));
                        }
                        section = MacroLambdaListSection::Optional;
                        index += 1;
                    }
                    "&REST" | "&BODY" => {
                        if lambda_list.rest.is_some()
                            || matches!(
                                section,
                                MacroLambdaListSection::Rest
                                    | MacroLambdaListSection::Keyword
                                    | MacroLambdaListSection::Auxiliary
                            )
                            || index + 1 >= parameters.len()
                        {
                            return Err(self.invalid(
                                "&rest or &body must be followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.rest = Some(self.macro_binding_name(
                            &parameters[index + 1],
                            seen,
                            destructuring,
                        )?);
                        section = MacroLambdaListSection::Rest;
                        index += 2;
                    }
                    "&KEY" => {
                        if lambda_list.has_keyword_section
                            || matches!(
                                section,
                                MacroLambdaListSection::Keyword | MacroLambdaListSection::Auxiliary
                            )
                        {
                            return Err(self.invalid(
                                "&key is out of order or repeated in macro lambda list",
                                parameter.span,
                            ));
                        }
                        lambda_list.has_keyword_section = true;
                        section = MacroLambdaListSection::Keyword;
                        index += 1;
                    }
                    "&ALLOW-OTHER-KEYS" => {
                        if section != MacroLambdaListSection::Keyword
                            || lambda_list.allow_other_keys
                        {
                            return Err(self.invalid(
                                "&allow-other-keys requires a keyword section",
                                parameter.span,
                            ));
                        }
                        lambda_list.allow_other_keys = true;
                        index += 1;
                    }
                    "&AUX" => {
                        if section == MacroLambdaListSection::Auxiliary {
                            return Err(self
                                .invalid("&aux is repeated in macro lambda list", parameter.span));
                        }
                        section = MacroLambdaListSection::Auxiliary;
                        index += 1;
                    }
                    "&ENVIRONMENT" => {
                        if lambda_list.environment.is_some() || index + 1 >= parameters.len() {
                            return Err(self.invalid(
                                "&environment must be followed by one parameter",
                                parameter.span,
                            ));
                        }
                        lambda_list.environment = Some(self.macro_binding_name(
                            &parameters[index + 1],
                            seen,
                            destructuring,
                        )?);
                        index += 2;
                    }
                    _ if marker.starts_with('&') => {
                        return Err(
                            self.invalid("unsupported marker in macro lambda list", parameter.span)
                        );
                    }
                    _ => {
                        if section == MacroLambdaListSection::Rest {
                            return Err(self.invalid(
                                "macro rest parameter must be followed by a keyword or auxiliary section",
                                parameter.span,
                            ));
                        }
                        match section {
                            MacroLambdaListSection::Required => {
                                lambda_list.required.push(self.macro_pattern(
                                    parameter,
                                    seen,
                                    destructuring,
                                )?);
                            }
                            MacroLambdaListSection::Optional => {
                                lambda_list
                                    .optional
                                    .push(self.parse_macro_optional_parameter(
                                        parameter,
                                        seen,
                                        destructuring,
                                    )?);
                            }
                            MacroLambdaListSection::Keyword => {
                                if lambda_list.allow_other_keys {
                                    return Err(self.invalid(
                                        "&allow-other-keys must be the last keyword-list marker",
                                        parameter.span,
                                    ));
                                }
                                let specification = self.parse_macro_keyword_parameter(
                                    parameter,
                                    seen,
                                    destructuring,
                                )?;
                                if lambda_list
                                    .keywords
                                    .iter()
                                    .any(|item| item.keyword_name == specification.keyword_name)
                                {
                                    return Err(self.invalid(
                                        "macro keyword names must be unique",
                                        parameter.span,
                                    ));
                                }
                                lambda_list.keywords.push(specification);
                            }
                            MacroLambdaListSection::Auxiliary => {
                                lambda_list
                                    .auxiliary
                                    .push(self.parse_macro_auxiliary_parameter(
                                        parameter,
                                        seen,
                                        destructuring,
                                    )?);
                            }
                            MacroLambdaListSection::Rest => unreachable!(),
                        }
                        index += 1;
                    }
                }
                continue;
            }

            if section == MacroLambdaListSection::Rest {
                return Err(self.invalid(
                    "macro rest parameter must be followed by a keyword or auxiliary section",
                    parameter.span,
                ));
            }
            match section {
                MacroLambdaListSection::Required => {
                    lambda_list.required.push(self.macro_pattern(
                        parameter,
                        seen,
                        destructuring,
                    )?);
                }
                MacroLambdaListSection::Optional => {
                    lambda_list
                        .optional
                        .push(self.parse_macro_optional_parameter(
                            parameter,
                            seen,
                            destructuring,
                        )?);
                }
                MacroLambdaListSection::Keyword => {
                    if lambda_list.allow_other_keys {
                        return Err(self.invalid(
                            "&allow-other-keys must be the last keyword-list marker",
                            parameter.span,
                        ));
                    }
                    let specification =
                        self.parse_macro_keyword_parameter(parameter, seen, destructuring)?;
                    if lambda_list
                        .keywords
                        .iter()
                        .any(|item| item.keyword_name == specification.keyword_name)
                    {
                        return Err(
                            self.invalid("macro keyword names must be unique", parameter.span)
                        );
                    }
                    lambda_list.keywords.push(specification);
                }
                MacroLambdaListSection::Auxiliary => {
                    lambda_list
                        .auxiliary
                        .push(self.parse_macro_auxiliary_parameter(
                            parameter,
                            seen,
                            destructuring,
                        )?);
                }
                MacroLambdaListSection::Rest => unreachable!(),
            }
            index += 1;
        }

        Ok(lambda_list)
    }

    fn macro_binding_name(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        destructuring: bool,
    ) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            let message = if destructuring {
                "destructuring parameter name must be a symbol"
            } else {
                "macro parameter must be a symbol"
            };
            return Err(self.invalid(message, form.span));
        };
        let normalized = normalize_name(name);
        if normalized.is_empty()
            || normalized.starts_with('&')
            || literal_atom(name).is_some()
            || !seen.insert(normalized.clone())
        {
            return Err(self.invalid(
                "macro parameter names must be unique and bindable",
                form.span,
            ));
        }
        Ok(normalized)
    }

    fn macro_pattern(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        destructuring: bool,
    ) -> Result<MacroPattern, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroPattern::Name(self.macro_binding_name(
                form,
                seen,
                destructuring,
            )?)),
            FormKind::List(items) => {
                if destructuring
                    && items.iter().any(|item| {
                        matches!(
                            &item.kind,
                            FormKind::Atom(name) if normalize_name(name).starts_with('&')
                        )
                    })
                {
                    Ok(MacroPattern::LambdaList(self.macro_parameters_with_seen(
                        form,
                        destructuring,
                        seen,
                    )?))
                } else {
                    Ok(MacroPattern::List(
                        items
                            .iter()
                            .map(|item| self.macro_pattern(item, seen, destructuring))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                }
            }
            FormKind::DottedList { items, tail } => Ok(MacroPattern::Dotted {
                items: items
                    .iter()
                    .map(|item| self.macro_pattern(item, seen, destructuring))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.macro_pattern(tail, seen, destructuring)?),
            }),
            _ => Err(self.invalid(
                "macro destructuring pattern must be a symbol or list",
                form.span,
            )),
        }
    }

    fn parse_macro_optional_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        destructuring: bool,
    ) -> Result<MacroOptionalParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroOptionalParameter {
                pattern: self.macro_pattern(form, seen, destructuring)?,
                init_form: nil(),
                supplied_p: None,
            }),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = self.macro_pattern(&items[0], seen, destructuring)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| {
                        self.macro_binding_name(item, seen, destructuring)
                            .map_err(|error| {
                                if destructuring {
                                    self.invalid(
                                        "destructuring supplied-p name must be a symbol",
                                        item.span,
                                    )
                                } else {
                                    error
                                }
                            })
                    })
                    .transpose()?;
                Ok(MacroOptionalParameter {
                    pattern,
                    init_form,
                    supplied_p,
                })
            }
            FormKind::List(_) => Err(self.invalid(
                "macro optional parameter must contain one to three items",
                form.span,
            )),
            _ => Err(self.invalid(
                "macro optional parameter must be a symbol or list",
                form.span,
            )),
        }
    }

    fn parse_macro_keyword_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        destructuring: bool,
    ) -> Result<MacroKeywordParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = self.macro_binding_name(form, seen, destructuring)?;
                let keyword_name = normalize_name(&name);
                (keyword_name, MacroPattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(self.invalid(
                            "macro keyword designator must contain a keyword and variable",
                            items[0].span,
                        ));
                    }
                    let Some(keyword_name) = macro_keyword_name(&key_specification[0]) else {
                        return Err(self.invalid(
                            if destructuring {
                                "destructuring keyword designator must be a symbol"
                            } else {
                                "macro keyword designator must start with a keyword"
                            },
                            key_specification[0].span,
                        ));
                    };
                    let pattern = self.macro_pattern(&key_specification[1], seen, destructuring)?;
                    (keyword_name, pattern, 1)
                } else if atom_name(&items[0]).is_some_and(|name| name.starts_with(':')) {
                    let Some(keyword_name) = macro_keyword_name(&items[0]) else {
                        return Err(self.invalid(
                            "macro keyword designator must be a nonempty keyword",
                            items[0].span,
                        ));
                    };
                    if items.len() < 2 {
                        return Err(
                            self.invalid("macro keyword parameter needs a variable", form.span)
                        );
                    }
                    let pattern = self.macro_pattern(&items[1], seen, destructuring)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = self.macro_pattern(&items[0], seen, destructuring)?;
                    let MacroPattern::Name(name) = &pattern else {
                        return Err(self.invalid(
                            "macro keyword parameter must have a variable name",
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(self.invalid(
                    "macro keyword parameter must be a symbol or list",
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
            return Err(self.invalid("macro keyword parameter contains too many items", form.span));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| self.macro_binding_name(item, seen, destructuring))
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        Ok(MacroKeywordParameter {
            keyword_name,
            pattern,
            init_form,
            supplied_p,
        })
    }

    fn parse_macro_auxiliary_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
        destructuring: bool,
    ) -> Result<MacroAuxiliaryParameter, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroAuxiliaryParameter {
                name: self.macro_binding_name(form, seen, destructuring)?,
                init_form: Form::atom("NIL", form.span),
            }),
            FormKind::List(items) if (1..=2).contains(&items.len()) => {
                Ok(MacroAuxiliaryParameter {
                    name: self.macro_binding_name(&items[0], seen, destructuring)?,
                    init_form: items
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| Form::atom("NIL", form.span)),
                })
            }
            FormKind::List(_) => Err(self.invalid(
                "macro auxiliary parameter must contain one or two items",
                form.span,
            )),
            _ => Err(self.invalid(
                "macro auxiliary parameter must be a symbol or list",
                form.span,
            )),
        }
    }


    };
}

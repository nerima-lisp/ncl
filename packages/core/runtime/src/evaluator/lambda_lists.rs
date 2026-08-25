use super::*;

impl Runtime {
    pub(super) fn parameters(&self, form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            self.invalid(&message, error.span)
        })
    }

    pub(super) fn macro_parameters(&self, form: &Form) -> Result<MacroLambdaList, RuntimeError> {
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
        let mut seen = HashSet::new();
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
                        lambda_list.whole =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
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
                        lambda_list.rest =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
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
                        lambda_list.environment =
                            Some(self.macro_binding_name(&parameters[index + 1], &mut seen)?);
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
                                lambda_list
                                    .required
                                    .push(self.macro_pattern(parameter, &mut seen)?);
                            }
                            MacroLambdaListSection::Optional => {
                                lambda_list.optional.push(
                                    self.parse_macro_optional_parameter(parameter, &mut seen)?,
                                );
                            }
                            MacroLambdaListSection::Keyword => {
                                if lambda_list.allow_other_keys {
                                    return Err(self.invalid(
                                        "&allow-other-keys must be the last keyword-list marker",
                                        parameter.span,
                                    ));
                                }
                                let specification =
                                    self.parse_macro_keyword_parameter(parameter, &mut seen)?;
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
                                lambda_list.auxiliary.push(
                                    self.parse_macro_auxiliary_parameter(parameter, &mut seen)?,
                                );
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
                    lambda_list
                        .required
                        .push(self.macro_pattern(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Optional => {
                    lambda_list
                        .optional
                        .push(self.parse_macro_optional_parameter(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Keyword => {
                    if lambda_list.allow_other_keys {
                        return Err(self.invalid(
                            "&allow-other-keys must be the last keyword-list marker",
                            parameter.span,
                        ));
                    }
                    let specification = self.parse_macro_keyword_parameter(parameter, &mut seen)?;
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
                        .push(self.parse_macro_auxiliary_parameter(parameter, &mut seen)?);
                }
                MacroLambdaListSection::Rest => unreachable!(),
            }
            index += 1;
        }

        Ok(lambda_list)
    }

    pub(super) fn macro_binding_name(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid("macro parameter must be a symbol", form.span));
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

    pub(super) fn macro_pattern(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroPattern, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroPattern::Name(self.macro_binding_name(form, seen)?)),
            FormKind::List(items) => Ok(MacroPattern::List(
                items
                    .iter()
                    .map(|item| self.macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(MacroPattern::Dotted {
                items: items
                    .iter()
                    .map(|item| self.macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(self.macro_pattern(tail, seen)?),
            }),
            _ => Err(self.invalid(
                "macro destructuring pattern must be a symbol or list",
                form.span,
            )),
        }
    }

    pub(super) fn parse_macro_optional_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroOptionalParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroOptionalParameter {
                pattern: self.macro_pattern(form, seen)?,
                init_form: nil(),
                supplied_p: None,
            }),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = self.macro_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| self.macro_binding_name(item, seen))
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

    pub(super) fn parse_macro_keyword_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroKeywordParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = self.macro_binding_name(form, seen)?;
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
                            "macro keyword designator must start with a keyword",
                            key_specification[0].span,
                        ));
                    };
                    let pattern = self.macro_pattern(&key_specification[1], seen)?;
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
                    let pattern = self.macro_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = self.macro_pattern(&items[0], seen)?;
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
                    .map(|item| self.macro_binding_name(item, seen))
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

    pub(super) fn parse_macro_auxiliary_parameter(
        &self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroAuxiliaryParameter, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroAuxiliaryParameter {
                name: self.macro_binding_name(form, seen)?,
                init_form: Form::atom("NIL", form.span),
            }),
            FormKind::List(items) if (1..=2).contains(&items.len()) => {
                Ok(MacroAuxiliaryParameter {
                    name: self.macro_binding_name(&items[0], seen)?,
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

    pub(super) fn eval_sequence_values(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::Nil;
        for form in forms {
            result = self.eval_values_in(form, environment)?;
        }
        Ok(result)
    }

    pub(crate) fn quoted_value(&self, form: &Form) -> Result<Value, RuntimeError> {
        quoted_form_value(form)
    }

    pub(crate) fn form_from_value(&self, value: &Value, span: Span) -> Result<Form, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(Form::atom("NIL", span)),
            Value::Boolean(true) => Ok(Form::atom("T", span)),
            Value::Integer(value) => Ok(Form::atom(value.to_string(), span)),
            Value::Rational(value) => Ok(Form::atom(
                format!("{}/{}", value.numerator(), value.denominator()),
                span,
            )),
            Value::Float(value) => Ok(Form::atom(value.to_string(), span)),
            Value::String(value) => Ok(Form::new(FormKind::String(value.to_string()), span)),
            Value::Character(value) => Ok(Form::new(FormKind::Character(*value), span)),
            Value::Package(name) => Ok(Form::list(
                vec![
                    Form::atom("FIND-PACKAGE", span),
                    Form::new(FormKind::String(name.to_string()), span),
                ],
                span,
            )),
            Value::Symbol(value) => Ok(Form::atom(value.as_ref(), span)),
            Value::SymbolExact(value) => Ok(Form::atom(escaped_symbol_atom(value), span)),
            Value::UninternedSymbol(value) => Ok(Form::atom(format!("#:{value}"), span)),
            Value::Keyword(value) => Ok(Form::atom(format!(":{value}"), span)),
            Value::KeywordExact(value) => {
                Ok(Form::atom(format!(":{}", escaped_symbol_atom(value)), span))
            }
            Value::List(values) => Ok(Form::list(
                values
                    .iter()
                    .map(|value| self.form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                span,
            )),
            Value::DottedList { items, tail } => Ok(Form::dotted_list(
                items
                    .iter()
                    .map(|value| self.form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                self.form_from_value(tail, span)?,
                span,
            )),
            Value::Vector(values) => Ok(Form::new(
                FormKind::Vector(
                    values
                        .iter()
                        .map(|value| self.form_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Value::Array { .. }
            | Value::HashTable { .. }
            | Value::Stream(_)
            | Value::Values(_)
            | Value::Condition(_)
            | Value::Restart(_)
            | Value::Unbound
            | Value::Environment(_)
            | Value::Class(_)
            | Value::Instance(_)
            | Value::Structure { .. } => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
            Value::Function(_) => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    pub(super) fn arity(&self, function: &str, expected: &str, actual: usize) -> RuntimeError {
        RuntimeError::Arity {
            function: function.to_string(),
            expected: expected.to_string(),
            actual,
        }
    }

    pub(super) fn block_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("block name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("block name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    pub(super) fn restart_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(self.invalid("restart name must be a symbol", form.span));
        };
        if name.is_empty() || (name.starts_with(':') && name.len() == 1) {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        if !name.starts_with(':')
            && literal_atom(name).is_some()
            && !name.eq_ignore_ascii_case("nil")
            && !name.eq_ignore_ascii_case("t")
        {
            return Err(self.invalid("restart name must be a symbol", form.span));
        }
        Ok(normalize_name(name))
    }

    pub(super) fn condition_name(&self, form: &Form) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid("condition name must be a symbol", form.span));
        };
        if name.is_empty()
            || (name.starts_with(':') && name.len() == 1)
            || (!name.starts_with(':')
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(self.invalid("condition name must be a symbol", form.span));
        }
        Ok(normalize_name(name).trim_start_matches(':').to_string())
    }

    pub(super) fn variable_name_info(
        &self,
        form: &Form,
        context: &str,
    ) -> Result<(String, bool), RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(self.invalid(context, form.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(self.invalid(context, form.span));
        };
        if token.kind != SymbolTokenKind::Symbol
            || token.name.is_empty()
            || (token.escaped && token.package.is_some())
            || (!token.escaped && (token.name.starts_with('&') || literal_atom(name).is_some()))
        {
            return Err(self.invalid(context, form.span));
        }
        let variable_name = if token.escaped {
            token.name
        } else {
            normalize_name(name)
        };
        Ok((variable_name, token.escaped))
    }

    pub(super) fn variable_name(&self, form: &Form, context: &str) -> Result<String, RuntimeError> {
        self.variable_name_info(form, context).map(|(name, _)| name)
    }

    pub(super) fn define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) {
        if escaped {
            self.define_exact_in(name, value, environment);
        } else {
            self.define_in(name, value, environment);
        }
    }

    pub(super) fn set_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) -> bool {
        if escaped {
            self.set_exact_in(name, value, environment)
        } else {
            self.set_in(name, value, environment)
        }
    }

    pub(super) fn set_or_define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if escaped {
            self.set_or_define_exact_in(name, value, environment, span)
        } else {
            self.set_or_define_in(name, value, environment, span)
        }
    }

    pub(super) fn ensure_symbol_writable(
        &self,
        name: &str,
        escaped: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let constant = if escaped {
            self.is_constant_exact_in(name)
        } else {
            self.is_constant_in(name)
        };
        if constant {
            Err(self.constant_modification_error(name, span))
        } else {
            Ok(())
        }
    }

    pub(super) fn invalid(&self, message: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: message.to_string(),
            span: Some(span),
        }
    }
}

#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn parameters(form: &Form) -> Result<OrdinaryLambdaList, RuntimeError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let message = error.kind.to_string();
            Self::invalid(&message, error.span)
        })
    }

    pub(super) fn handle_macro_marker(
        lambda_list: &mut MacroLambdaList,
        section: &mut MacroLambdaListSection,
        parameters: &[Form],
        index: usize,
        parameter: &Form,
        marker: &str,
        seen: &mut HashSet<String>,
    ) -> Result<Option<usize>, RuntimeError> {
        let next = match marker {
            "&WHOLE" => {
                if index != 0 || lambda_list.whole.is_some() || index + 1 >= parameters.len() {
                    return Err(Self::invalid(
                        "&whole must be the first marker followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.whole = Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                index + 2
            }
            "&OPTIONAL" => {
                if *section != MacroLambdaListSection::Required {
                    return Err(Self::invalid(
                        "&optional is out of order in macro lambda list",
                        parameter.span,
                    ));
                }
                *section = MacroLambdaListSection::Optional;
                index + 1
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
                    return Err(Self::invalid(
                        "&rest or &body must be followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.rest = Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                *section = MacroLambdaListSection::Rest;
                index + 2
            }
            "&KEY" => {
                if lambda_list.has_keyword_section
                    || matches!(
                        section,
                        MacroLambdaListSection::Keyword | MacroLambdaListSection::Auxiliary
                    )
                {
                    return Err(Self::invalid(
                        "&key is out of order or repeated in macro lambda list",
                        parameter.span,
                    ));
                }
                lambda_list.has_keyword_section = true;
                *section = MacroLambdaListSection::Keyword;
                index + 1
            }
            "&ALLOW-OTHER-KEYS" => {
                if *section != MacroLambdaListSection::Keyword || lambda_list.allow_other_keys {
                    return Err(Self::invalid(
                        "&allow-other-keys requires a keyword section",
                        parameter.span,
                    ));
                }
                lambda_list.allow_other_keys = true;
                index + 1
            }
            "&AUX" => {
                if *section == MacroLambdaListSection::Auxiliary {
                    return Err(Self::invalid(
                        "&aux is repeated in macro lambda list",
                        parameter.span,
                    ));
                }
                *section = MacroLambdaListSection::Auxiliary;
                index + 1
            }
            "&ENVIRONMENT" => {
                if lambda_list.environment.is_some() || index + 1 >= parameters.len() {
                    return Err(Self::invalid(
                        "&environment must be followed by one parameter",
                        parameter.span,
                    ));
                }
                lambda_list.environment =
                    Some(Self::macro_binding_name(&parameters[index + 1], seen)?);
                index + 2
            }
            _ if marker.starts_with('&') => {
                return Err(Self::invalid(
                    "unsupported marker in macro lambda list",
                    parameter.span,
                ));
            }
            _ => return Ok(None),
        };
        Ok(Some(next))
    }

    pub(crate) fn macro_parameters(form: &Form) -> Result<MacroLambdaList, RuntimeError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(Self::invalid("macro parameters must be a list", form.span));
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
                if let Some(next_index) = Self::handle_macro_marker(
                    &mut lambda_list,
                    &mut section,
                    parameters,
                    index,
                    parameter,
                    &marker,
                    &mut seen,
                )? {
                    index = next_index;
                    continue;
                }
            }

            Self::push_macro_parameter(&mut lambda_list, section, parameter, &mut seen)?;
            index += 1;
        }

        Ok(lambda_list)
    }

    pub(super) fn push_macro_parameter(
        lambda_list: &mut MacroLambdaList,
        section: MacroLambdaListSection,
        parameter: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<(), RuntimeError> {
        if section == MacroLambdaListSection::Rest {
            return Err(Self::invalid(
                "macro rest parameter must be followed by a keyword or auxiliary section",
                parameter.span,
            ));
        }
        match section {
            MacroLambdaListSection::Required => {
                lambda_list
                    .required
                    .push(Self::macro_pattern(parameter, seen)?);
            }
            MacroLambdaListSection::Optional => {
                lambda_list
                    .optional
                    .push(Self::parse_macro_optional_parameter(parameter, seen)?);
            }
            MacroLambdaListSection::Keyword => {
                if lambda_list.allow_other_keys {
                    return Err(Self::invalid(
                        "&allow-other-keys must be the last keyword-list marker",
                        parameter.span,
                    ));
                }
                let specification = Self::parse_macro_keyword_parameter(parameter, seen)?;
                if lambda_list
                    .keywords
                    .iter()
                    .any(|item| item.keyword_name == specification.keyword_name)
                {
                    return Err(Self::invalid(
                        "macro keyword names must be unique",
                        parameter.span,
                    ));
                }
                lambda_list.keywords.push(specification);
            }
            MacroLambdaListSection::Auxiliary => {
                lambda_list
                    .auxiliary
                    .push(Self::parse_macro_auxiliary_parameter(parameter, seen)?);
            }
            MacroLambdaListSection::Rest => unreachable!(),
        }
        Ok(())
    }

    pub(super) fn macro_binding_name(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<String, RuntimeError> {
        let Some(name) = atom_name(form) else {
            return Err(Self::invalid("macro parameter must be a symbol", form.span));
        };
        let normalized = normalize_name(name);
        if normalized.is_empty()
            || normalized.starts_with('&')
            || literal_atom(name).is_some()
            || !seen.insert(normalized.clone())
        {
            return Err(Self::invalid(
                "macro parameter names must be unique and bindable",
                form.span,
            ));
        }
        Ok(normalized)
    }

    pub(super) fn macro_pattern(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroPattern, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroPattern::Name(Self::macro_binding_name(form, seen)?)),
            FormKind::List(items) => Ok(MacroPattern::List(
                items
                    .iter()
                    .map(|item| Self::macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            FormKind::DottedList { items, tail } => Ok(MacroPattern::Dotted {
                items: items
                    .iter()
                    .map(|item| Self::macro_pattern(item, seen))
                    .collect::<Result<Vec<_>, _>>()?,
                tail: Box::new(Self::macro_pattern(tail, seen)?),
            }),
            _ => Err(Self::invalid(
                "macro destructuring pattern must be a symbol or list",
                form.span,
            )),
        }
    }

    pub(super) fn parse_macro_optional_parameter(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroOptionalParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroOptionalParameter {
                pattern: Self::macro_pattern(form, seen)?,
                init_form: nil(),
                supplied_p: None,
            }),
            FormKind::List(items) if (1..=3).contains(&items.len()) => {
                let pattern = Self::macro_pattern(&items[0], seen)?;
                let init_form = items.get(1).cloned().unwrap_or_else(nil);
                let supplied_p = items
                    .get(2)
                    .map(|item| Self::macro_binding_name(item, seen))
                    .transpose()?;
                Ok(MacroOptionalParameter {
                    pattern,
                    init_form,
                    supplied_p,
                })
            }
            FormKind::List(_) => Err(Self::invalid(
                "macro optional parameter must contain one to three items",
                form.span,
            )),
            _ => Err(Self::invalid(
                "macro optional parameter must be a symbol or list",
                form.span,
            )),
        }
    }

    pub(super) fn parse_macro_keyword_parameter(
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroKeywordParameter, RuntimeError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = Self::macro_binding_name(form, seen)?;
                let keyword_name = normalize_name(&name);
                (keyword_name, MacroPattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(Self::invalid(
                            "macro keyword designator must contain a keyword and variable",
                            items[0].span,
                        ));
                    }
                    let Some(keyword_name) = macro_keyword_name(&key_specification[0]) else {
                        return Err(Self::invalid(
                            "macro keyword designator must start with a keyword",
                            key_specification[0].span,
                        ));
                    };
                    let pattern = Self::macro_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if atom_name(&items[0]).is_some_and(|name| name.starts_with(':')) {
                    let Some(keyword_name) = macro_keyword_name(&items[0]) else {
                        return Err(Self::invalid(
                            "macro keyword designator must be a nonempty keyword",
                            items[0].span,
                        ));
                    };
                    if items.len() < 2 {
                        return Err(Self::invalid(
                            "macro keyword parameter needs a variable",
                            form.span,
                        ));
                    }
                    let pattern = Self::macro_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = Self::macro_pattern(&items[0], seen)?;
                    let MacroPattern::Name(name) = &pattern else {
                        return Err(Self::invalid(
                            "macro keyword parameter must have a variable name",
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(Self::invalid(
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
            return Err(Self::invalid(
                "macro keyword parameter contains too many items",
                form.span,
            ));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| Self::macro_binding_name(item, seen))
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
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<MacroAuxiliaryParameter, RuntimeError> {
        match &form.kind {
            FormKind::Atom(_) => Ok(MacroAuxiliaryParameter {
                name: Self::macro_binding_name(form, seen)?,
                init_form: Form::atom("NIL", form.span),
            }),
            FormKind::List(items) if (1..=2).contains(&items.len()) => {
                Ok(MacroAuxiliaryParameter {
                    name: Self::macro_binding_name(&items[0], seen)?,
                    init_form: items
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| Form::atom("NIL", form.span)),
                })
            }
            FormKind::List(_) => Err(Self::invalid(
                "macro auxiliary parameter must contain one or two items",
                form.span,
            )),
            _ => Err(Self::invalid(
                "macro auxiliary parameter must be a symbol or list",
                form.span,
            )),
        }
    }

    pub(crate) fn eval_sequence_values(
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

    pub(crate) fn quoted_value(form: &Form) -> Result<Value, RuntimeError> {
        quoted_form_value(form)
    }

    pub(crate) fn form_from_value(value: &Value, span: Span) -> Result<Form, RuntimeError> {
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
                    .map(|value| Self::form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                span,
            )),
            Value::DottedList { items, tail } => Ok(Form::dotted_list(
                items
                    .iter()
                    .map(|value| Self::form_from_value(value, span))
                    .collect::<Result<Vec<_>, _>>()?,
                Self::form_from_value(tail, span)?,
                span,
            )),
            Value::Vector(values) => Ok(Form::new(
                FormKind::Vector(
                    values
                        .iter()
                        .map(|value| Self::form_from_value(value, span))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                span,
            )),
            Value::Array { .. }
            | Value::HashTable { .. }
            | Value::Stream(_)
            | Value::RandomState(_)
            | Value::Values(_)
            | Value::Condition(_)
            | Value::Restart(_)
            | Value::Unbound
            | Value::Environment(_)
            | Value::Class(_)
            | Value::Instance(_)
            | Value::Structure { .. }
            | Value::Function(_) => Err(RuntimeError::Type {
                expected: "FORM".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        }
    }
}

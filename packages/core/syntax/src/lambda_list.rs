use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::{Form, FormKind, Span, SymbolTokenKind, parse_symbol_token};

/// The ordinary lambda-list shape shared by the compiler and evaluator.
#[derive(Clone, Debug, PartialEq)]
pub struct OrdinaryLambdaList {
    /// Required parameter names.
    pub required: Vec<String>,
    /// Whether each required name used escaping.
    pub required_escaped: Vec<bool>,
    /// Optional parameters.
    pub optional: Vec<LambdaListOptionalParameter>,
    /// Rest parameter name.
    pub rest: Option<String>,
    /// Whether the rest name used escaping.
    pub rest_escaped: bool,
    /// Keyword parameters.
    pub keywords: Vec<LambdaListKeywordParameter>,
    /// Whether an `&KEY` section was present.
    pub has_keyword_section: bool,
    /// Whether unknown keywords are accepted.
    pub allow_other_keys: bool,
    /// Auxiliary parameters.
    pub auxiliary: Vec<LambdaListAuxiliaryParameter>,
}

/// One `&OPTIONAL` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListOptionalParameter {
    /// Parameter name.
    pub name: String,
    /// Whether the name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
    /// Whether an initialization form was explicitly supplied.
    pub init_form_supplied: bool,
    /// `supplied-p` variable name.
    pub supplied_p: Option<String>,
    /// Whether the `supplied-p` name used escaping.
    pub supplied_p_escaped: Option<bool>,
}

/// One `&KEY` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListKeywordParameter {
    /// External keyword name.
    pub keyword_name: String,
    /// Whether the keyword name used escaping.
    pub keyword_name_escaped: bool,
    /// Local parameter name.
    pub name: String,
    /// Whether the local name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
    /// Whether an initialization form was explicitly supplied.
    pub init_form_supplied: bool,
    /// `supplied-p` variable name.
    pub supplied_p: Option<String>,
    /// Whether the `supplied-p` name used escaping.
    pub supplied_p_escaped: Option<bool>,
}

/// One `&AUX` parameter specification.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaListAuxiliaryParameter {
    /// Parameter name.
    pub name: String,
    /// Whether the name used escaping.
    pub name_escaped: bool,
    /// Initialization form.
    pub init_form: Form,
}

/// The category of an ordinary lambda-list syntax error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LambdaListErrorKind {
    /// The parameter form was not a proper list.
    ExpectedList,
    /// A symbol was required in the named context.
    ExpectedSymbol {
        /// Parameter-list context.
        context: &'static str,
    },
    /// The form violated lambda-list syntax.
    InvalidForm {
        /// Human-readable validation detail.
        message: String,
    },
}

/// A lambda-list syntax error tied to the offending source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaListError {
    /// Error category.
    pub kind: LambdaListErrorKind,
    /// Source location of the error.
    pub span: Span,
}

impl LambdaListError {
    const fn expected_symbol(context: &'static str, span: Span) -> Self {
        Self {
            kind: LambdaListErrorKind::ExpectedSymbol { context },
            span,
        }
    }

    fn invalid(message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: LambdaListErrorKind::InvalidForm {
                message: message.into(),
            },
            span,
        }
    }
}

impl fmt::Display for LambdaListErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedList => formatter.write_str("parameters must be a list"),
            Self::ExpectedSymbol { context } => write!(formatter, "{context} must be a symbol"),
            Self::InvalidForm { message } => formatter.write_str(message),
        }
    }
}

impl fmt::Display for LambdaListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for LambdaListError {}

/// Parse the Common Lisp ordinary lambda-list subset supported by ncl.
///
/// The accepted grammar is:
///
/// ```text
/// lambda-list ::= required* [&OPTIONAL optional-spec*] [&REST name]
///                [&KEY keyword-spec* [&ALLOW-OTHER-KEYS]]
///                [&AUX auxiliary-spec*]
/// optional-spec ::= name | (name init-form [supplied-p])
/// keyword-spec ::= name | (name init-form [supplied-p])
///                | ((keyword-name name) init-form [supplied-p])
/// auxiliary-spec ::= name | (name init-form)
/// ```
///
/// Other lambda-list markers are rejected here so that the compiler and the
/// interpreter fail consistently instead of silently treating a marker as a
/// variable name.
///
/// # Errors
///
/// Returns [`LambdaListError`] when the form does not follow the ordinary
/// lambda-list grammar.
#[allow(clippy::too_many_lines)]
pub fn parse_ordinary_lambda_list(form: &Form) -> Result<OrdinaryLambdaList, LambdaListError> {
    let parameters: &[Form] = match &form.kind {
        FormKind::List(parameters) => parameters,
        // Runtime values represent the empty list as NIL.  This case is
        // needed when a quoted lambda form is reconstructed for EVAL/COMPILE.
        FormKind::Atom(name) if name == "NIL" => &[],
        _ => {
            return Err(LambdaListError {
                kind: LambdaListErrorKind::ExpectedList,
                span: form.span,
            });
        }
    };

    let mut required = Vec::new();
    let mut required_escaped = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut rest_escaped = false;
    let mut keywords = Vec::new();
    let mut has_keyword_section = false;
    let mut allow_other_keys = false;
    let mut auxiliary = Vec::new();
    let mut names = HashSet::new();
    let mut keyword_names = HashSet::new();
    let mut section = LambdaListSection::Required;
    let mut index = 0;

    while index < parameters.len() {
        let parameter = &parameters[index];
        if let Some(marker) = marker_name(parameter) {
            match marker.as_str() {
                "&OPTIONAL" => {
                    if !matches!(section, LambdaListSection::Required) {
                        return Err(LambdaListError::invalid(
                            "&optional may appear only once at the beginning of the lambda-list",
                            parameter.span,
                        ));
                    }
                    section = LambdaListSection::Optional;
                    index += 1;
                    continue;
                }
                "&REST" => {
                    if rest.is_some()
                        || matches!(
                            section,
                            LambdaListSection::Keyword | LambdaListSection::Auxiliary
                        )
                    {
                        return Err(LambdaListError::invalid(
                            "&rest may appear only once before &key or &aux",
                            parameter.span,
                        ));
                    }
                    let Some(rest_parameter) = parameters.get(index + 1) else {
                        return Err(LambdaListError::invalid(
                            "&rest must be followed by one parameter",
                            parameter.span,
                        ));
                    };
                    if marker_name(rest_parameter).is_some_and(|name| name.starts_with('&')) {
                        return Err(LambdaListError::invalid(
                            "&rest must be followed by one parameter",
                            rest_parameter.span,
                        ));
                    }
                    let (rest_name, escaped) = parse_name(rest_parameter, "&rest parameter")?;
                    insert_unique(&mut names, &rest_name, rest_parameter.span)?;
                    rest = Some(rest_name);
                    rest_escaped = escaped;
                    section = LambdaListSection::Rest;
                    index += 2;
                    continue;
                }
                "&KEY" => {
                    if has_keyword_section || matches!(section, LambdaListSection::Auxiliary) {
                        return Err(LambdaListError::invalid(
                            "&key may appear only once before &aux",
                            parameter.span,
                        ));
                    }
                    has_keyword_section = true;
                    section = LambdaListSection::Keyword;
                    index += 1;
                    continue;
                }
                "&ALLOW-OTHER-KEYS" => {
                    if !has_keyword_section
                        || !matches!(section, LambdaListSection::Keyword)
                        || allow_other_keys
                    {
                        return Err(LambdaListError::invalid(
                            "&allow-other-keys requires one &key section and may appear only once",
                            parameter.span,
                        ));
                    }
                    allow_other_keys = true;
                    index += 1;
                    continue;
                }
                "&AUX" => {
                    if matches!(section, LambdaListSection::Auxiliary) {
                        return Err(LambdaListError::invalid(
                            "&aux may appear only once",
                            parameter.span,
                        ));
                    }
                    section = LambdaListSection::Auxiliary;
                    index += 1;
                    continue;
                }
                _ if marker.starts_with('&') => {
                    return Err(LambdaListError::invalid(
                        format!("unsupported lambda-list marker {marker}"),
                        parameter.span,
                    ));
                }
                _ => {}
            }
        }

        match section {
            LambdaListSection::Required => {
                let (name, escaped) = parse_name(parameter, "parameter")?;
                insert_unique(&mut names, &name, parameter.span)?;
                required.push(name);
                required_escaped.push(escaped);
            }
            LambdaListSection::Optional => {
                let specification = parse_optional_parameter(parameter)?;
                insert_unique(&mut names, &specification.name, parameter.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    insert_unique(&mut names, supplied_p, parameter.span)?;
                }
                optional.push(specification);
            }
            LambdaListSection::Rest => {
                return Err(LambdaListError::invalid(
                    "&rest must be followed by &key, &aux, or end of lambda-list",
                    parameter.span,
                ));
            }
            LambdaListSection::Keyword => {
                if allow_other_keys {
                    return Err(LambdaListError::invalid(
                        "&allow-other-keys must be the last item in the &key section",
                        parameter.span,
                    ));
                }
                let specification = parse_keyword_parameter(parameter)?;
                if !keyword_names.insert(specification.keyword_name.clone()) {
                    return Err(LambdaListError::invalid(
                        "keyword names must be unique",
                        parameter.span,
                    ));
                }
                insert_unique(&mut names, &specification.name, parameter.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    insert_unique(&mut names, supplied_p, parameter.span)?;
                }
                keywords.push(specification);
            }
            LambdaListSection::Auxiliary => {
                let specification = parse_auxiliary_parameter(parameter)?;
                insert_unique(&mut names, &specification.name, parameter.span)?;
                auxiliary.push(specification);
            }
        }
        index += 1;
    }

    Ok(OrdinaryLambdaList {
        required,
        required_escaped,
        optional,
        rest,
        rest_escaped,
        keywords,
        has_keyword_section,
        allow_other_keys,
        auxiliary,
    })
}

fn insert_unique(
    names: &mut HashSet<String>,
    name: &str,
    span: Span,
) -> Result<(), LambdaListError> {
    if names.insert(name.to_owned()) {
        Ok(())
    } else {
        Err(LambdaListError::invalid(
            "parameter names must be unique",
            span,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

fn parse_optional_parameter(form: &Form) -> Result<LambdaListOptionalParameter, LambdaListError> {
    match &form.kind {
        FormKind::Atom(_) => {
            let (name, name_escaped) = parse_name(form, "optional parameter")?;
            Ok(LambdaListOptionalParameter {
                name,
                name_escaped,
                init_form: Form::atom("NIL", form.span),
                init_form_supplied: false,
                supplied_p: None,
                supplied_p_escaped: None,
            })
        }
        FormKind::List(items) if (1..=3).contains(&items.len()) => {
            let (name, name_escaped) = parse_name(&items[0], "optional parameter")?;
            let init_form = items
                .get(1)
                .cloned()
                .unwrap_or_else(|| Form::atom("NIL", form.span));
            let (supplied_p, supplied_p_escaped) = items
                .get(2)
                .map(|supplied_p| parse_name(supplied_p, "supplied-p parameter"))
                .transpose()?
                .map_or((None, None), |(name, escaped)| (Some(name), Some(escaped)));
            Ok(LambdaListOptionalParameter {
                name,
                name_escaped,
                init_form,
                init_form_supplied: items.get(1).is_some(),
                supplied_p,
                supplied_p_escaped,
            })
        }
        FormKind::List(_) => Err(LambdaListError::invalid(
            "optional parameter must contain one to three elements",
            form.span,
        )),
        _ => Err(LambdaListError::expected_symbol(
            "optional parameter",
            form.span,
        )),
    }
}

fn parse_keyword_parameter(form: &Form) -> Result<LambdaListKeywordParameter, LambdaListError> {
    let (
        keyword_name,
        keyword_name_escaped,
        name,
        name_escaped,
        init_form,
        init_form_supplied,
        supplied_p,
        supplied_p_escaped,
    ) = match &form.kind {
        FormKind::Atom(_) => {
            let (name, name_escaped) = parse_name(form, "keyword parameter")?;
            (
                name.clone(),
                name_escaped,
                name,
                name_escaped,
                Form::atom("NIL", form.span),
                false,
                None,
                None,
            )
        }
        FormKind::List(items) if (1..=3).contains(&items.len()) => {
            let (keyword_name, keyword_name_escaped, name, name_escaped) = match &items[0].kind {
                FormKind::Atom(_) => {
                    let (name, name_escaped) = parse_name(&items[0], "keyword parameter")?;
                    (name.clone(), name_escaped, name, name_escaped)
                }
                FormKind::List(keyword_specification) if keyword_specification.len() == 2 => {
                    let (keyword_name, keyword_name_escaped) =
                        parse_keyword_name(&keyword_specification[0], "keyword name")?;
                    let (name, name_escaped) =
                        parse_name(&keyword_specification[1], "keyword parameter")?;
                    (keyword_name, keyword_name_escaped, name, name_escaped)
                }
                FormKind::List(_) => {
                    return Err(LambdaListError::invalid(
                        "keyword name and parameter must contain two elements",
                        items[0].span,
                    ));
                }
                _ => {
                    return Err(LambdaListError::expected_symbol(
                        "keyword parameter",
                        items[0].span,
                    ));
                }
            };
            let init_form = items
                .get(1)
                .cloned()
                .unwrap_or_else(|| Form::atom("NIL", form.span));
            let supplied_p = items
                .get(2)
                .map(|supplied_p| parse_name(supplied_p, "supplied-p parameter"))
                .transpose()?;
            let (supplied_p, supplied_p_escaped) =
                supplied_p.map_or((None, None), |(name, escaped)| (Some(name), Some(escaped)));
            (
                keyword_name,
                keyword_name_escaped,
                name,
                name_escaped,
                init_form,
                items.get(1).is_some(),
                supplied_p,
                supplied_p_escaped,
            )
        }
        FormKind::List(_) => {
            return Err(LambdaListError::invalid(
                "keyword parameter must contain one to three elements",
                form.span,
            ));
        }
        _ => {
            return Err(LambdaListError::expected_symbol(
                "keyword parameter",
                form.span,
            ));
        }
    };

    Ok(LambdaListKeywordParameter {
        keyword_name,
        keyword_name_escaped,
        name,
        name_escaped,
        init_form,
        init_form_supplied,
        supplied_p,
        supplied_p_escaped,
    })
}

fn parse_keyword_name(
    form: &Form,
    context: &'static str,
) -> Result<(String, bool), LambdaListError> {
    let FormKind::Atom(name) = &form.kind else {
        return Err(LambdaListError::expected_symbol(context, form.span));
    };
    let Ok(token) = parse_symbol_token(name) else {
        return Err(LambdaListError::expected_symbol(context, form.span));
    };
    if token.name.is_empty()
        || token.package.is_some()
        || token.kind == SymbolTokenKind::Uninterned
        || (!token.escaped
            && token.kind == SymbolTokenKind::Symbol
            && (token.name.starts_with('&') || literal_atom(name)))
    {
        return Err(LambdaListError::expected_symbol(context, form.span));
    }
    Ok(if token.escaped {
        (token.name, true)
    } else {
        (normalize_name(&token.name), false)
    })
}

fn parse_auxiliary_parameter(form: &Form) -> Result<LambdaListAuxiliaryParameter, LambdaListError> {
    match &form.kind {
        FormKind::Atom(_) => {
            let (name, name_escaped) = parse_name(form, "auxiliary parameter")?;
            Ok(LambdaListAuxiliaryParameter {
                name,
                name_escaped,
                init_form: Form::atom("NIL", form.span),
            })
        }
        FormKind::List(items) if (1..=2).contains(&items.len()) => {
            let (name, name_escaped) = parse_name(&items[0], "auxiliary parameter")?;
            let init_form = items
                .get(1)
                .cloned()
                .unwrap_or_else(|| Form::atom("NIL", form.span));
            Ok(LambdaListAuxiliaryParameter {
                name,
                name_escaped,
                init_form,
            })
        }
        FormKind::List(_) => Err(LambdaListError::invalid(
            "auxiliary parameter must contain one or two elements",
            form.span,
        )),
        _ => Err(LambdaListError::expected_symbol(
            "auxiliary parameter",
            form.span,
        )),
    }
}

fn parse_name(form: &Form, context: &'static str) -> Result<(String, bool), LambdaListError> {
    let FormKind::Atom(name) = &form.kind else {
        return Err(LambdaListError::expected_symbol(context, form.span));
    };
    let Ok(token) = parse_symbol_token(name) else {
        return Err(LambdaListError::expected_symbol(context, form.span));
    };
    if token.kind != SymbolTokenKind::Symbol
        || token.name.is_empty()
        || (token.escaped && token.package.is_some())
        || (!token.escaped && (token.name.starts_with('&') || literal_atom(name)))
    {
        return Err(LambdaListError::expected_symbol(context, form.span));
    }
    Ok(if token.escaped {
        (token.name, true)
    } else {
        (normalize_name(name), false)
    })
}

fn marker_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let token = parse_symbol_token(name).ok()?;
    if token.kind != SymbolTokenKind::Symbol || token.package.is_some() || token.escaped {
        return None;
    }
    Some(normalize_name(&token.name))
}

fn atom_name(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(name) => Some(name),
        _ => None,
    }
}

fn literal_atom(name: &str) -> bool {
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    if token.kind == SymbolTokenKind::Keyword {
        return true;
    }
    if token.kind != SymbolTokenKind::Symbol || token.package.is_some() || token.escaped {
        return false;
    }
    token.name == "NIL"
        || token.name == "T"
        || token.name == "#F"
        || token.name == "#T"
        || token.name.parse::<i64>().is_ok()
        || token.name.parse::<f64>().is_ok()
}

fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

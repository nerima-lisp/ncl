use crate::lambda_list_types::{LambdaListError, LambdaListKeywordParameter};
use crate::{Form, FormKind, SymbolTokenKind, parse_symbol_token};

use super::names::{literal_atom, normalize_name, parse_name};

pub(super) fn parse_keyword_parameter(
    form: &Form,
) -> Result<LambdaListKeywordParameter, LambdaListError> {
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

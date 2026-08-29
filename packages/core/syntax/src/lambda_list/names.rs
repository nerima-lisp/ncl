use crate::lambda_list_types::LambdaListError;
use crate::{Form, FormKind, SymbolTokenKind, parse_symbol_token};

pub(super) fn parse_name(
    form: &Form,
    context: &'static str,
) -> Result<(String, bool), LambdaListError> {
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

pub(super) fn marker_name(form: &Form) -> Option<String> {
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

pub(super) fn literal_atom(name: &str) -> bool {
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

/// Upper-cases `name` to the canonical form NCL uses for case-insensitive
/// symbol comparisons.
#[must_use]
pub fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

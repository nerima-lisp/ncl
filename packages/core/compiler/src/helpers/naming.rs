use crate::{Form, FormKind, Span, SymbolTokenKind, parse_symbol_token};

use super::literals::literal_constant;

pub(super) fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

pub(super) fn operator_span(items: &[Form], fallback: Span) -> Span {
    items.first().map_or(fallback, |form| form.span)
}

pub(super) fn symbol_reference(atom: &str) -> Option<(String, bool)> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind != SymbolTokenKind::Symbol {
        return None;
    }
    if token.escaped {
        return token.package.is_none().then_some((token.name, true));
    }
    Some((normalize_name(atom), false))
}

pub(super) fn special_operator_name(atom: &str) -> Option<String> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind == SymbolTokenKind::Symbol && token.package.is_none() && !token.escaped {
        Some(normalize_name(&token.name))
    } else {
        None
    }
}

pub(super) fn case_default_clause(form: &Form) -> bool {
    let FormKind::Atom(atom) = &form.kind else {
        return false;
    };
    let Ok(token) = parse_symbol_token(atom) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && (token.name.eq_ignore_ascii_case("T") || token.name.eq_ignore_ascii_case("OTHERWISE"))
}

pub(super) fn tag_name(form: &Form) -> Option<String> {
    let FormKind::Atom(name) = &form.kind else {
        return None;
    };
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_constant(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_and_special_operator_classification_preserves_escaping() {
        let symbol_cases = [
            ("name", Some(("NAME".to_string(), false))),
            ("|Exact|", Some(("Exact".to_string(), true))),
            (":keyword", None),
            ("#\\x", Some(("#x".to_string(), true))),
        ];
        for (source, expected) in symbol_cases {
            assert_eq!(symbol_reference(source), expected, "source={source:?}");
        }

        let operator_cases = [("if", Some("IF")), ("|if|", None), (":if", None)];
        for (source, expected) in operator_cases {
            assert_eq!(
                special_operator_name(source).as_deref(),
                expected,
                "source={source:?}"
            );
        }
    }

    #[test]
    fn case_default_clause_matches_t_and_otherwise() {
        let span = Span::new(0, 1);
        let cases = [
            (Form::atom("t", span), true),
            (Form::atom("otherwise", span), true),
            (Form::atom("|t|", span), false),
            (Form::list(Vec::new(), span), false),
        ];
        for (form, expected) in cases {
            assert_eq!(case_default_clause(&form), expected);
        }
    }

    #[test]
    fn tag_names_filter_literals_and_normalize_symbols() {
        let span = Span::new(0, 1);
        let cases = [
            ("done", Some("DONE")),
            (":done", Some(":DONE")),
            ("nil", Some("NIL")),
            ("1", Some("1")),
            ("t", Some("T")),
            (":", None),
            ("", None),
            ("3/4", None),
        ];
        for (source, expected) in cases {
            assert_eq!(tag_name(&Form::atom(source, span)).as_deref(), expected);
        }
    }
}

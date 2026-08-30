use super::form_predicates::atom_name;
use super::{Form, SymbolTokenKind, literal_atom, normalize_name, package, parse_symbol_token};
use crate::environment::{intern_name, names_equal};

pub(in crate::evaluator) fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    package::split_symbol(&normalized)
        .map(|(_, symbol, _)| symbol.to_string())
        .unwrap_or(normalized)
}

fn is_unqualified_name_in(name: &str, expected: &[&str]) -> bool {
    let normalized = intern_name(name);
    let candidate = package::split_symbol(normalized.as_ref())
        .map_or_else(|| normalized.as_ref(), |(_, symbol, _)| symbol);
    expected
        .iter()
        .any(|expected| names_equal(candidate, expected))
}

pub(in crate::evaluator) fn is_special_operator_name(name: &str) -> bool {
    is_unqualified_name_in(
        name,
        &[
            "BLOCK",
            "CATCH",
            "EVAL-WHEN",
            "FLET",
            "FUNCTION",
            "GO",
            "IF",
            "LABELS",
            "LET",
            "LET*",
            "LOAD-TIME-VALUE",
            "LOCALLY",
            "MACROLET",
            "MULTIPLE-VALUE-CALL",
            "MULTIPLE-VALUE-PROG1",
            "PROGN",
            "PROGV",
            "QUOTE",
            "SETQ",
            "SYMBOL-MACROLET",
            "TAGBODY",
            "THE",
            "THROW",
            "UNWIND-PROTECT",
        ],
    )
}

pub(in crate::evaluator) fn is_case_default_form(form: &Form) -> bool {
    let Some(name) = atom_name(form) else {
        return false;
    };
    let Ok(token) = parse_symbol_token(name) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && is_unqualified_name_in(name, &["T", "OTHERWISE"])
}

pub(in crate::evaluator) fn control_tag(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_atom(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, Span};

    use super::{control_tag, is_case_default_form};

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn case_default_form_rejects_a_non_atom_clause_key() {
        let key_list = Form::list(vec![Form::atom("1", SPAN)], SPAN);
        assert!(!is_case_default_form(&key_list));
    }

    #[test]
    fn control_tag_rejects_a_bare_colon() {
        assert!(control_tag(&Form::atom(":", SPAN)).is_none());
    }

    #[test]
    fn control_tag_rejects_an_empty_atom() {
        assert!(control_tag(&Form::atom("", SPAN)).is_none());
    }
}

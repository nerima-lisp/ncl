use super::form_predicates::atom_name;
use super::{Form, SymbolTokenKind, literal_atom, normalize_name, package, parse_symbol_token};

pub(in crate::evaluator) fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    package::split_symbol(&normalized)
        .map(|(_, symbol, _)| symbol.to_string())
        .unwrap_or(normalized)
}

pub(in crate::evaluator) fn is_special_operator_name(name: &str) -> bool {
    matches!(unqualified_name(name).as_str(), |"BLOCK"| "CATCH"
        | "EVAL-WHEN"
        | "FLET"
        | "FUNCTION"
        | "GO"
        | "IF"
        | "LABELS"
        | "LET"
        | "LET*"
        | "LOAD-TIME-VALUE"
        | "LOCALLY"
        | "MACROLET"
        | "MULTIPLE-VALUE-CALL"
        | "MULTIPLE-VALUE-PROG1"
        | "PROGN"
        | "PROGV"
        | "QUOTE"
        | "SETQ"
        | "SYMBOL-MACROLET"
        | "TAGBODY"
        | "THE"
        | "THROW"
        | "UNWIND-PROTECT")
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
        && matches!(unqualified_name(name).as_str(), "T" | "OTHERWISE")
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

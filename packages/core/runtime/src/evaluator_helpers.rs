#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn atom_name(form: &Form) -> Option<&str> {
    match &form.kind {
        FormKind::Atom(value) => Some(value),
        _ => None,
    }
}

pub(super) fn is_nil_form(form: &Form) -> bool {
    atom_name(form).is_some_and(|name| name.eq_ignore_ascii_case("nil"))
}

pub(super) fn is_macro_keyword_form(form: &Form) -> bool {
    macro_keyword_name(form).is_some()
}

pub(super) fn macro_keyword_name(form: &Form) -> Option<String> {
    let name = atom_name(form)?;
    let keyword = name.strip_prefix(':')?;
    (!keyword.is_empty()).then(|| normalize_name(keyword))
}

pub(super) fn macro_dotted_parts(value: &Value) -> Option<(Vec<Value>, Value)> {
    match value {
        Value::Nil => Some((Vec::new(), Value::Nil)),
        Value::List(values) => Some((values.as_ref().clone(), Value::Nil)),
        Value::DottedList { items, tail } => {
            let mut values = items.as_ref().clone();
            match tail.as_ref() {
                Value::Nil => Some((values, Value::Nil)),
                Value::List(more) => {
                    values.extend(more.as_ref().iter().cloned());
                    Some((values, Value::Nil))
                }
                Value::DottedList { .. } => {
                    let (more, dotted_tail) = macro_dotted_parts(tail)?;
                    values.extend(more);
                    Some((values, dotted_tail))
                }
                other => Some((values, other.clone())),
            }
        }
        _ => None,
    }
}

pub(super) fn control_tag(form: &Form) -> Option<String> {
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

pub(super) fn unqualified_name(name: &str) -> String {
    let normalized = normalize_name(name);
    package::split_symbol(&normalized)
        .map(|(_, symbol, _)| symbol.to_string())
        .unwrap_or(normalized)
}

pub(super) fn is_special_operator_name(name: &str) -> bool {
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

pub(super) fn is_case_default_form(form: &Form) -> bool {
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

pub(super) fn is_operator_form(form: &Form, name: &str) -> bool {
    match &form.kind {
        FormKind::List(items) => items
            .first()
            .and_then(atom_name)
            .is_some_and(|operator| operator.eq_ignore_ascii_case(name)),
        _ => false,
    }
}

pub(super) const SPECIAL_FORM_NAMES: &[&str] = &[
    "QUOTE",
    "QUASIQUOTE",
    "DECLARE",
    "LOCALLY",
    "EVAL-WHEN",
    "LOAD-TIME-VALUE",
    "NTH-VALUE",
    "DECLAIM",
    "PROCLAIM",
    "THE",
    "IF",
    "PROGN",
    "PROG1",
    "PROG2",
    "PROG",
    "PROG*",
    "VALUES",
    "IGNORE-ERRORS",
    "HANDLER-CASE",
    "HANDLER-BIND",
    "RESTART-BIND",
    "WITH-CONDITION-RESTARTS",
    "CATCH",
    "PROGV",
    "THROW",
    "WITH-SIMPLE-RESTART",
    "WITH-OPEN-FILE",
    "RESTART-CASE",
    "UNWIND-PROTECT",
    "BLOCK",
    "RETURN",
    "RETURN-FROM",
    "TAGBODY",
    "GO",
    "MULTIPLE-VALUE-BIND",
    "MULTIPLE-VALUE-CALL",
    "MULTIPLE-VALUE-LIST",
    "MULTIPLE-VALUE-PROG1",
    "AND",
    "OR",
    "WHEN",
    "UNLESS",
    "COND",
    "CASE",
    "ECASE",
    "TYPECASE",
    "ETYPECASE",
    "DESTRUCTURING-BIND",
    "LET",
    "LET*",
    "FLET",
    "LABELS",
    "MACROLET",
    "SYMBOL-MACROLET",
    "DOTIMES",
    "DOLIST",
    "DO",
    "DO*",
    "LAMBDA",
    "FUNCTION",
    "DEFUN",
    "DEFMACRO",
    "DEFINE-MODIFY-MACRO",
    "MACROEXPAND-1",
    "MACROEXPAND",
    "DEFPACKAGE",
    "IN-PACKAGE",
    "DEFINE",
    "DEFINE-SYMBOL-MACRO",
    "SETQ",
    "PSETQ",
    "MULTIPLE-VALUE-SETQ",
    "SETF",
    "PSETF",
    "PUSH",
    "POP",
    "PUSHNEW",
    "ROTATEF",
    "SHIFTF",
    "DEFSETF",
    "INCF",
    "DECF",
    "DEFSTRUCT",
    "DEFCLASS",
    "DEFGENERIC",
    "DEFMETHOD",
    "DEFVAR",
    "DEFPARAMETER",
    "DEFCONSTANT",
    "DEFINE-SETF-EXPANDER",
    "GET-SETF-EXPANSION",
    "EVAL",
    "FUNCALL",
    "APPLY",
    "MAP-INTO",
    "MAPCAR",
];

pub(super) fn is_special_form(form: &Form) -> bool {
    atom_name(form)
        .is_some_and(|operator| SPECIAL_FORM_NAMES.contains(&normalize_name(operator).as_str()))
}

pub(super) fn prefix_argument<'form>(items: &'form [Form], name: &str) -> Option<&'form Form> {
    if items.len() != 2 {
        return None;
    }
    atom_name(&items[0]).filter(|operator| operator.eq_ignore_ascii_case(name))?;
    Some(&items[1])
}

pub(super) fn quasiquote_marker(name: &str, value: Value) -> Value {
    Value::list(vec![Value::symbol(name), value])
}

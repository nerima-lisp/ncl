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

pub(super) fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::Nil => Some(Vec::new()),
        Value::List(items) | Value::Vector(items) => Some(items.as_ref().clone()),
        Value::String(value) => Some(value.chars().map(Value::Character).collect()),
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
    matches!(
        unqualified_name(name).as_str(),
        "BLOCK"
            | "CATCH"
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
            | "UNWIND-PROTECT"
    )
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

pub(super) fn is_special_form(form: &Form) -> bool {
    let Some(operator) = atom_name(form) else {
        return false;
    };
    matches!(
        normalize_name(operator).as_str(),
        "QUOTE"
            | "QUASIQUOTE"
            | "DECLARE"
            | "LOCALLY"
            | "EVAL-WHEN"
            | "LOAD-TIME-VALUE"
            | "NTH-VALUE"
            | "DECLAIM"
            | "PROCLAIM"
            | "THE"
            | "IF"
            | "PROGN"
            | "PROG1"
            | "PROG2"
            | "PROG"
            | "PROG*"
            | "VALUES"
            | "IGNORE-ERRORS"
            | "HANDLER-CASE"
            | "HANDLER-BIND"
            | "RESTART-BIND"
            | "WITH-CONDITION-RESTARTS"
            | "CATCH"
            | "PROGV"
            | "THROW"
            | "WITH-SIMPLE-RESTART"
            | "WITH-OPEN-FILE"
            | "RESTART-CASE"
            | "UNWIND-PROTECT"
            | "BLOCK"
            | "RETURN"
            | "RETURN-FROM"
            | "TAGBODY"
            | "GO"
            | "MULTIPLE-VALUE-BIND"
            | "MULTIPLE-VALUE-CALL"
            | "MULTIPLE-VALUE-LIST"
            | "MULTIPLE-VALUE-PROG1"
            | "AND"
            | "OR"
            | "WHEN"
            | "UNLESS"
            | "COND"
            | "CASE"
            | "ECASE"
            | "TYPECASE"
            | "ETYPECASE"
            | "DESTRUCTURING-BIND"
            | "LET"
            | "LET*"
            | "FLET"
            | "LABELS"
            | "MACROLET"
            | "SYMBOL-MACROLET"
            | "DOTIMES"
            | "DOLIST"
            | "DO"
            | "DO*"
            | "LAMBDA"
            | "FUNCTION"
            | "DEFUN"
            | "DEFMACRO"
            | "DEFINE-MODIFY-MACRO"
            | "MACROEXPAND-1"
            | "MACROEXPAND"
            | "DEFPACKAGE"
            | "IN-PACKAGE"
            | "DEFINE"
            | "DEFINE-SYMBOL-MACRO"
            | "SETQ"
            | "PSETQ"
            | "MULTIPLE-VALUE-SETQ"
            | "SETF"
            | "PSETF"
            | "PUSH"
            | "POP"
            | "PUSHNEW"
            | "ROTATEF"
            | "SHIFTF"
            | "DEFSETF"
            | "INCF"
            | "DECF"
            | "DEFSTRUCT"
            | "DEFCLASS"
            | "DEFGENERIC"
            | "DEFMETHOD"
            | "DEFVAR"
            | "DEFPARAMETER"
            | "DEFCONSTANT"
            | "DEFINE-SETF-EXPANDER"
            | "GET-SETF-EXPANSION"
            | "EVAL"
            | "FUNCALL"
            | "APPLY"
            | "MAP-INTO"
            | "MAPCAR"
    )
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

pub(crate) fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
    match &form.kind {
        FormKind::Atom(atom) => {
            if let Ok(token) = parse_symbol_token(atom) {
                match token.kind {
                    SymbolTokenKind::Uninterned => return Ok(Value::uninterned_symbol(token.name)),
                    SymbolTokenKind::Keyword => {
                        return Ok(if token.escaped {
                            Value::keyword_exact(token.name)
                        } else {
                            Value::keyword(token.name)
                        });
                    }
                    SymbolTokenKind::Symbol => {
                        if let Some(package) = token.package {
                            let name = format!("{}::{}", normalize_name(&package), token.name);
                            return Ok(if token.escaped {
                                Value::symbol_exact(name)
                            } else {
                                Value::symbol(name)
                            });
                        }
                        if token.escaped {
                            return Ok(Value::symbol_exact(token.name));
                        }
                    }
                }
            }
            Ok(literal_atom(atom).unwrap_or_else(|| Value::symbol(atom)))
        }
        FormKind::String(value) => Ok(Value::string(value.clone())),
        FormKind::Character(value) => Ok(Value::Character(*value)),
        FormKind::List(items) => Ok(Value::list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        FormKind::DottedList { items, tail } => Ok(Value::dotted_list(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
            quoted_form_value(tail)?,
        )),
        FormKind::Vector(items) => Ok(Value::vector(
            items
                .iter()
                .map(quoted_form_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
    }
}

pub(super) fn escaped_symbol_atom(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('|');
    for character in value.chars() {
        if matches!(character, '|' | '\\') {
            result.push('\\');
        }
        result.push(character);
    }
    result.push('|');
    result
}

pub(super) fn literal_atom(atom: &str) -> Option<Value> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => Some(if token.escaped {
            Value::keyword_exact(token.name)
        } else {
            Value::keyword(token.name)
        }),
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            match token.name.as_str() {
                "NIL" | "#F" => return Some(Value::Nil),
                "T" | "#T" => return Some(Value::boolean(true)),
                _ => {}
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Value::Integer(value));
            }
            if let Some((numerator, denominator)) = token.name.split_once('/')
                && let (Ok(numerator), Ok(denominator)) =
                    (numerator.parse::<i128>(), denominator.parse::<i128>())
            {
                return Value::rational(numerator, denominator).ok();
            }
            token.name.parse::<f64>().ok().map(Value::Float)
        }
        _ => None,
    }
}

pub(super) fn resolved_symbol(atom: &str) -> (String, bool) {
    let Ok(token) = parse_symbol_token(atom) else {
        return (normalize_name(atom), false);
    };
    match token.kind {
        SymbolTokenKind::Uninterned => (format!("#:{}", token.name), token.escaped),
        SymbolTokenKind::Keyword => (format!(":{}", token.name), token.escaped),
        SymbolTokenKind::Symbol => {
            let name = if token.escaped {
                token.name
            } else {
                normalize_name(&token.name)
            };
            let resolved = token.package.map_or(name.clone(), |package| {
                package::canonical_symbol_name(&package, &name)
            });
            (resolved, token.escaped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use ncl_syntax::{Form, Span};

    fn atom(name: &str) -> Form {
        Form::atom(name, Span::new(0, name.len()))
    }

    #[test]
    fn sequence_items_normalizes_supported_sequence_shapes() {
        let cases = [
            (Value::Nil, Vec::new()),
            (
                Value::list(vec![Value::Integer(1)]),
                vec![Value::Integer(1)],
            ),
            (
                Value::vector(vec![Value::Integer(2)]),
                vec![Value::Integer(2)],
            ),
            (
                Value::string("ab"),
                vec![Value::Character('a'), Value::Character('b')],
            ),
        ];

        for (value, expected) in cases {
            let actual = sequence_items(&value).expect("supported sequence");
            assert_eq!(actual.len(), expected.len());
            assert!(
                actual
                    .iter()
                    .zip(expected.iter())
                    .all(|(actual, expected)| actual.equal_value(expected))
            );
        }
    }

    #[test]
    fn sequence_items_rejects_non_sequence_values() {
        assert!(sequence_items(&Value::Integer(1)).is_none());
    }

    #[test]
    fn symbol_helpers_handle_packages_and_escaping() {
        assert_eq!(macro_keyword_name(&atom(":when")), Some("WHEN".into()));
        assert_eq!(macro_keyword_name(&atom(":")), None);
        assert_eq!(unqualified_name("pkg:name"), "NAME");
        assert_eq!(resolved_symbol(":|hello|"), (":hello".into(), true));
        assert_eq!(resolved_symbol("#:tmp"), ("#:TMP".into(), false));
        assert_eq!(escaped_symbol_atom("a|b\\c"), "|a\\|b\\\\c|");
    }

    #[test]
    fn control_and_operator_helpers_reject_invalid_forms() {
        assert_eq!(control_tag(&atom(":tag")), Some(":TAG".into()));
        assert_eq!(control_tag(&atom("1")), Some("1".into()));
        assert_eq!(control_tag(&atom("1.5")), None);
        assert!(is_special_operator_name("pkg:if"));
        assert!(!is_special_operator_name("unknown"));
        assert!(is_case_default_form(&atom("otherwise")));
        assert!(!is_case_default_form(&atom("|T|")));
        assert!(is_operator_form(
            &Form::list(vec![atom("+"), atom("1")], Span::new(0, 1)),
            "+"
        ));
        assert!(!is_operator_form(&atom("+"), "+"));
        let arguments = [atom("quote"), atom("x")];
        assert_eq!(
            atom_name(prefix_argument(&arguments, "QUOTE").unwrap()),
            Some("x")
        );
    }

    #[test]
    fn dotted_values_and_quoted_forms_preserve_shape() {
        let dotted = Value::dotted_list(
            vec![Value::Integer(1)],
            Value::list(vec![Value::Integer(2), Value::Integer(3)]),
        );
        let (items, tail) = macro_dotted_parts(&dotted).unwrap();
        assert!(
            items
                .iter()
                .zip([1, 2, 3])
                .all(|(value, expected)| { value.equal_value(&Value::Integer(expected)) })
        );
        assert!(tail.equal_value(&Value::Nil));
        let quoted = Form::dotted_list(vec![atom("x")], atom(".tail"), Span::new(0, 1));
        assert!(quoted_form_value(&quoted).is_ok());
        let marker = quasiquote_marker("QUOTE", Value::Integer(1));
        let expected = Value::list(vec![Value::symbol("QUOTE"), Value::Integer(1)]);
        assert!(marker.equal_value(&expected));
    }
}

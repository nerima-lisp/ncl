use ncl_syntax::{Form, FormKind, Span, SymbolTokenKind, parse_symbol_token};

use super::data::Constant;
use super::{CompileError, CompileErrorKind};

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

pub(super) fn compile_eval_when_executes(form: &Form) -> Result<bool, CompileError> {
    let FormKind::List(situations) = &form.kind else {
        return Err(CompileError::new(
            CompileErrorKind::ExpectedList {
                context: "EVAL-WHEN situations".to_string(),
            },
            form.span,
        ));
    };
    let mut executes = false;
    for situation in situations {
        let FormKind::Atom(name) = &situation.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        if token.kind == SymbolTokenKind::Uninterned
            || (token.kind == SymbolTokenKind::Symbol && literal_constant(name).is_some())
        {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        }
        if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
            executes = true;
        }
    }
    Ok(executes)
}

pub(super) fn literal_constant(atom: &str) -> Option<Constant> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => {
            if token.escaped {
                Some(Constant::KeywordExact(token.name))
            } else {
                Some(Constant::Keyword(normalize_name(&token.name)))
            }
        }
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            if token.name.eq_ignore_ascii_case("nil") || token.name.eq_ignore_ascii_case("#f") {
                return Some(Constant::Nil);
            }
            if token.name.eq_ignore_ascii_case("t") || token.name.eq_ignore_ascii_case("#t") {
                return Some(Constant::Boolean(true));
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Constant::Integer(value));
            }
            if let Some((numerator, denominator)) = rational_literal_parts(&token.name) {
                return if denominator == 1 {
                    Some(Constant::Integer(numerator))
                } else {
                    Some(Constant::Rational {
                        numerator,
                        denominator,
                    })
                };
            }
            token.name.parse::<f64>().ok().map(Constant::Float)
        }
        _ => None,
    }
}

pub(super) fn rational_literal_parts(name: &str) -> Option<(i64, i64)> {
    let (numerator, denominator) = name.split_once('/')?;
    if numerator.is_empty()
        || denominator.is_empty()
        || numerator.contains('/')
        || denominator.contains('/')
    {
        return None;
    }
    let numerator = numerator.parse::<i128>().ok()?;
    let denominator = denominator.parse::<i128>().ok()?;
    if denominator == 0 {
        return None;
    }
    let (numerator, denominator) = if denominator < 0 {
        (numerator.checked_neg()?, denominator.checked_neg()?)
    } else {
        (numerator, denominator)
    };
    let numerator_abs = if numerator < 0 {
        numerator.checked_neg()? as u128
    } else {
        numerator as u128
    };
    let divisor = gcd(numerator_abs, denominator as u128);
    let numerator = i64::try_from(numerator / divisor as i128).ok()?;
    let denominator = i64::try_from(denominator / divisor as i128).ok()?;
    Some((numerator, denominator))
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
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

    fn atom(name: &str) -> Form {
        Form::atom(name, Span::new(2, 4))
    }

    #[test]
    fn names_and_spans_follow_symbol_rules() {
        assert_eq!(normalize_name("MiXeD"), "MIXED");
        assert_eq!(operator_span(&[], Span::new(8, 9)), Span::new(8, 9));
        assert_eq!(
            operator_span(&[atom("op")], Span::new(8, 9)),
            Span::new(2, 4)
        );
        assert_eq!(symbol_reference("name"), Some(("NAME".into(), false)));
        assert_eq!(symbol_reference("|name|"), Some(("name".into(), true)));
        assert_eq!(symbol_reference(":name"), None);
        assert_eq!(
            symbol_reference("pkg:name"),
            Some(("PKG:NAME".into(), false))
        );
        assert_eq!(special_operator_name("eval"), Some("EVAL".into()));
        assert_eq!(special_operator_name(":eval"), None);
        assert_eq!(special_operator_name("|eval|"), None);
    }

    #[test]
    fn case_defaults_and_tags_reject_non_default_forms() {
        for name in ["t", "T", "otherwise", "OTHERWISE"] {
            assert!(case_default_clause(&atom(name)));
        }
        for form in [Form::list(vec![], Span::new(0, 1)), atom("|T|"), atom(":x")] {
            assert!(!case_default_clause(&form));
        }
        assert_eq!(tag_name(&atom(":tag")), Some(":TAG".into()));
        assert_eq!(tag_name(&atom("label")), Some("LABEL".into()));
        assert_eq!(tag_name(&atom("")), None);
        assert_eq!(tag_name(&atom("nil")), Some("NIL".into()));
        assert_eq!(tag_name(&atom("42")), Some("42".into()));
        assert_eq!(tag_name(&atom("1.5")), None);
        assert_eq!(tag_name(&atom("1/2")), None);
        assert_eq!(tag_name(&atom(":")), None);
        assert_eq!(tag_name(&Form::list(vec![], Span::new(0, 1))), None);
    }

    #[test]
    fn literal_constants_cover_numeric_boolean_and_keyword_cases() {
        let cases = [
            ("nil", Constant::Nil),
            ("#f", Constant::Nil),
            ("t", Constant::Boolean(true)),
            ("#t", Constant::Boolean(true)),
            ("42", Constant::Integer(42)),
            (
                "6/4",
                Constant::Rational {
                    numerator: 3,
                    denominator: 2,
                },
            ),
            ("2/1", Constant::Integer(2)),
            ("1.5", Constant::Float(1.5)),
            (":hello", Constant::Keyword("HELLO".into())),
            (":|hello|", Constant::KeywordExact("hello".into())),
        ];
        for (source, expected) in cases {
            assert_eq!(literal_constant(source), Some(expected), "{source}");
        }
        assert_eq!(literal_constant("pkg:name"), None);
        assert_eq!(rational_literal_parts("1/-2"), Some((-1, 2)));
        assert_eq!(rational_literal_parts("1/0"), None);
        assert_eq!(rational_literal_parts("1/2/3"), None);
    }

    #[test]
    fn eval_when_requires_a_list_of_non_literal_symbols() {
        let span = Span::new(0, 3);
        assert_eq!(
            compile_eval_when_executes(&Form::list(vec![atom("execute")], span)),
            Ok(true)
        );
        assert_eq!(
            compile_eval_when_executes(&Form::list(vec![atom("load")], span)),
            Ok(false)
        );
        assert!(matches!(
            compile_eval_when_executes(&atom("execute")),
            Err(CompileError {
                kind: CompileErrorKind::ExpectedList { .. },
                ..
            })
        ));
        assert!(
            compile_eval_when_executes(&Form::list(vec![Form::list(vec![], span)], span)).is_err()
        );
        assert!(compile_eval_when_executes(&Form::list(vec![atom("t")], span)).is_err());
    }
}

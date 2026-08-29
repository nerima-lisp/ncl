#![allow(clippy::redundant_pub_crate)]
#[allow(clippy::wildcard_imports)]
use super::*;

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
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Constant::Integer(value));
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
            parse_float_literal(&token.name).map(Constant::Float)
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
    let numerator_abs = numerator.unsigned_abs();
    let denominator_abs = u128::try_from(denominator).ok()?;
    let divisor = gcd(numerator_abs, denominator_abs);
    let reduced_numerator = i128::try_from(numerator_abs / divisor).ok()?;
    let reduced_numerator = if numerator < 0 {
        reduced_numerator.checked_neg()?
    } else {
        reduced_numerator
    };
    let reduced_denominator = i128::try_from(denominator_abs / divisor).ok()?;
    Some((
        i64::try_from(reduced_numerator).ok()?,
        i64::try_from(reduced_denominator).ok()?,
    ))
}

pub(super) const fn gcd(mut left: u128, mut right: u128) -> u128 {
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

    #[test]
    fn literal_constants_are_parsed_from_a_table() {
        let cases = [
            ("nil", Constant::Nil),
            ("#t", Constant::Boolean(true)),
            (":ready", Constant::Keyword("READY".to_string())),
            (
                "1/2",
                Constant::Rational {
                    numerator: 1,
                    denominator: 2,
                },
            ),
            ("-6/3", Constant::Integer(-2)),
            ("#xFF", Constant::Integer(255)),
            ("#b1010", Constant::Integer(10)),
            ("#o777", Constant::Integer(511)),
            ("#3r120", Constant::Integer(15)),
            ("1.25s0", Constant::Float(1.25)),
        ];

        for (source, expected) in cases {
            assert_eq!(
                literal_constant(source),
                Some(expected),
                "source={source:?}"
            );
        }
    }

    #[test]
    fn rational_literals_cover_invalid_and_reduced_forms() {
        let cases = [
            ("6/8", Some((3, 4))),
            ("6/-8", Some((-3, 4))),
            ("0/9", Some((0, 1))),
            ("1/0", None),
            ("1/2/3", None),
            ("9223372036854775808/1", None),
        ];

        for (source, expected) in cases {
            assert_eq!(
                rational_literal_parts(source),
                expected,
                "source={source:?}"
            );
        }
    }

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
    fn eval_when_and_case_helpers_reject_non_symbol_forms() {
        let span = Span::new(0, 1);
        let list = Form::list(vec![Form::atom("execute", span)], span);
        assert_eq!(compile_eval_when_executes(&list), Ok(true));
        assert!(compile_eval_when_executes(&Form::atom("execute", span)).is_err());

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

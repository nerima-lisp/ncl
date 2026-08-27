use ncl_syntax::{Form, FormKind, SymbolTokenKind, parse_symbol_token};

use crate::environment::normalize_name;
use crate::package;
use crate::{RuntimeError, Value};

pub fn quoted_form_value(form: &Form) -> Result<Value, RuntimeError> {
    match &form.kind {
        FormKind::Atom(atom) => {
            if let Ok(token) = parse_symbol_token(atom) {
                match token.kind {
                    SymbolTokenKind::Uninterned => {
                        return Ok(Value::uninterned_symbol(token.name));
                    }
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

pub fn escaped_symbol_atom(value: &str) -> String {
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

pub fn literal_atom(atom: &str) -> Option<Value> {
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

pub fn resolved_symbol(atom: &str) -> (String, bool) {
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
            let resolved = token.package.map_or_else(
                || name.clone(),
                |package| package::canonical_symbol_name(&package, &name),
            );
            (resolved, token.escaped)
        }
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, FormKind, Span};

    use super::{escaped_symbol_atom, literal_atom, quoted_form_value, resolved_symbol};

    #[test]
    fn quoted_values_cover_composite_and_escaped_literals() {
        let span = Span::new(0, 1);
        let cases = [
            (Form::atom("|name|", span), "|name|"),
            (Form::atom(":|key|", span), ":|key|"),
            (Form::atom("#:|temporary|", span), "#:temporary"),
            (Form::atom("pkg:|name|", span), "|PKG::name|"),
            (Form::new(FormKind::String("text".into()), span), "\"text\""),
            (Form::new(FormKind::Character('x'), span), "#\\x"),
            (Form::list(vec![Form::atom("1", span)], span), "(1)"),
            (
                Form::dotted_list(vec![Form::atom("1", span)], Form::atom("2", span), span),
                "(1 . 2)",
            ),
            (
                Form::new(FormKind::Vector(vec![Form::atom("1", span)]), span),
                "#(1)",
            ),
        ];

        for (form, expected) in cases {
            let value = match quoted_form_value(&form) {
                Ok(value) => value,
                Err(error) => panic!("literal form failed: {error}"),
            };
            assert_eq!(value.to_string(), expected);
        }
    }

    #[test]
    fn literal_atoms_cover_language_boundaries() {
        let cases = [
            ("nil", "NIL"),
            ("#f", "NIL"),
            ("t", "T"),
            ("#t", "T"),
            ("42", "42"),
            ("3/6", "1/2"),
            ("1.5", "1.5"),
            (":name", ":NAME"),
        ];

        for (source, expected) in cases {
            let actual =
                literal_atom(source).map_or_else(|| "<none>".to_owned(), |value| value.to_string());
            assert_eq!(actual, expected, "{source}");
        }

        for source in ["(not-an-atom)", "|escaped|"] {
            assert!(literal_atom(source).is_none(), "{source}");
        }
    }

    #[test]
    fn resolved_symbols_preserve_package_and_escape_identity() {
        let cases = [
            ("name", ("NAME", false)),
            ("|name|", ("name", true)),
            (":key", (":KEY", false)),
            ("#:temporary", ("#:TEMPORARY", false)),
            ("pkg:name", ("PKG::NAME", false)),
        ];

        for (source, expected) in cases {
            assert_eq!(
                resolved_symbol(source),
                (expected.0.to_owned(), expected.1),
                "{source}"
            );
        }
    }

    #[test]
    fn escaped_symbol_atoms_quote_delimiters() {
        assert_eq!(escaped_symbol_atom("a|b\\c"), "|a\\|b\\\\c|");
    }
}

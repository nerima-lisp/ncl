use ncl_syntax::{
    SymbolTokenKind, parse_float_literal, parse_radix_integer_literal, parse_symbol_token,
};

use crate::Value;

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
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Value::Integer(value));
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
            parse_float_literal(&token.name).map(Value::Float)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::literal_atom;

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
            ("#xFF", "255"),
            ("#b1010", "10"),
            ("#o777", "511"),
            ("#3r120", "15"),
            ("1.25s0", "1.25"),
            ("1.25d0", "1.25"),
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
}

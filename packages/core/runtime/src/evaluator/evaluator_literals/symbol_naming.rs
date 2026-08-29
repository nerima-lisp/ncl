use ncl_syntax::{SymbolTokenKind, parse_symbol_token};

use crate::environment::normalize_name;
use crate::package;

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
    use super::{escaped_symbol_atom, resolved_symbol};

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

use super::{SymbolTokenError, SymbolTokenKind, parse_symbol_token};

#[test]
fn parses_symbol_namespaces_and_escaping() {
    let cases = [
        ("name", SymbolTokenKind::Symbol, None, "NAME", false, false),
        (":name", SymbolTokenKind::Keyword, None, "NAME", true, false),
        (
            "pkg:name",
            SymbolTokenKind::Symbol,
            Some("PKG"),
            "NAME",
            true,
            false,
        ),
        (
            "pkg::name",
            SymbolTokenKind::Symbol,
            Some("PKG"),
            "NAME",
            false,
            false,
        ),
        (
            "#:name",
            SymbolTokenKind::Uninterned,
            None,
            "NAME",
            false,
            false,
        ),
        (
            "|MiXeD|",
            SymbolTokenKind::Symbol,
            None,
            "MiXeD",
            false,
            true,
        ),
        (
            "pkg:|Mi|",
            SymbolTokenKind::Symbol,
            Some("PKG"),
            "Mi",
            true,
            true,
        ),
    ];

    for (input, kind, package, name, external, escaped) in cases {
        let token = parse_symbol_token(input).unwrap_or_else(|error| panic!("{input}: {error}"));
        assert_eq!(token.kind, kind, "{input}");
        assert_eq!(token.package.as_deref(), package, "{input}");
        assert_eq!(token.name, name, "{input}");
        assert_eq!(token.external, external, "{input}");
        assert_eq!(token.escaped, escaped, "{input}");
    }
}

#[test]
fn accepts_empty_escaped_symbol_names() {
    let token = parse_symbol_token("||").expect("empty escaped symbol should be valid");

    assert_eq!(token.kind, SymbolTokenKind::Symbol);
    assert_eq!(token.package, None);
    assert_eq!(token.name, "");
    assert!(!token.external);
    assert!(token.escaped);
}

#[test]
fn rejects_invalid_symbol_tokens() {
    let cases = [
        ("", SymbolTokenError::EmptyName),
        ("\\", SymbolTokenError::UnterminatedEscape),
        ("|name", SymbolTokenError::UnterminatedEscape),
        ("#:", SymbolTokenError::EmptyName),
        ("pkg:", SymbolTokenError::EmptyName),
        ("#:name:extra", SymbolTokenError::InvalidQualifier),
        ("::name", SymbolTokenError::InvalidQualifier),
        ("pkg:::name", SymbolTokenError::InvalidQualifier),
    ];

    for (input, expected) in cases {
        assert_eq!(parse_symbol_token(input), Err(expected), "{input}");
    }
}

#[test]
fn formats_symbol_token_errors() {
    let cases = [
        (
            SymbolTokenError::UnterminatedEscape,
            "unterminated symbol escape",
        ),
        (SymbolTokenError::EmptyName, "symbol name is empty"),
        (
            SymbolTokenError::InvalidQualifier,
            "invalid symbol package qualifier",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn decodes_escaped_delimiters_without_treating_them_as_qualifiers() {
    let token = parse_symbol_token(r"pkg\:name").unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(token.kind, SymbolTokenKind::Symbol);
    assert_eq!(token.package, None);
    assert_eq!(token.name, "PKG:NAME");
    assert!(!token.external);
    assert!(token.escaped);
}

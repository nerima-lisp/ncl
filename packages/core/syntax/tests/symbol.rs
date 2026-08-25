use ncl_syntax::{SymbolTokenError, SymbolTokenKind, parse_symbol_token};

#[test]
fn parses_symbol_qualifiers_and_escapes() {
    let plain = parse_symbol_token("foo").unwrap();
    assert_eq!(plain.kind, SymbolTokenKind::Symbol);
    assert_eq!(plain.name, "FOO");
    assert_eq!(plain.package, None);

    let keyword = parse_symbol_token(":foo").unwrap();
    assert_eq!(keyword.kind, SymbolTokenKind::Keyword);
    assert_eq!(keyword.name, "FOO");
    assert!(keyword.external);

    let external = parse_symbol_token("pkg:foo").unwrap();
    assert_eq!(external.package.as_deref(), Some("PKG"));
    assert_eq!(external.name, "FOO");
    assert!(external.external);

    let internal = parse_symbol_token("pkg::foo").unwrap();
    assert_eq!(internal.package.as_deref(), Some("PKG"));
    assert_eq!(internal.name, "FOO");
    assert!(!internal.external);

    let escaped = parse_symbol_token("|MiXeD|\\:").unwrap();
    assert_eq!(escaped.name, "MiXeD:");
    assert!(escaped.escaped);
}

#[test]
fn parses_uninterned_symbols_and_rejects_invalid_qualifiers() {
    let uninterned = parse_symbol_token("#:foo").unwrap();
    assert_eq!(uninterned.kind, SymbolTokenKind::Uninterned);
    assert_eq!(uninterned.name, "FOO");

    for input in ["", ":", "pkg:", "pkg:::foo", "#:foo:bar"] {
        assert_eq!(
            parse_symbol_token(input).unwrap_err(),
            if input.is_empty() || input == ":" || input == "pkg:" {
                SymbolTokenError::EmptyName
            } else {
                SymbolTokenError::InvalidQualifier
            }
        );
    }
}

#[test]
fn reports_unterminated_escapes() {
    assert_eq!(
        parse_symbol_token("foo\\").unwrap_err(),
        SymbolTokenError::UnterminatedEscape
    );
    assert_eq!(
        parse_symbol_token("|foo").unwrap_err(),
        SymbolTokenError::UnterminatedEscape
    );
}

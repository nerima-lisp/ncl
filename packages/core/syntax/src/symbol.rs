use std::error::Error;
use std::fmt;

/// The namespace represented by a symbol token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolTokenKind {
    /// An ordinary symbol.
    Symbol,
    /// A keyword symbol.
    Keyword,
    /// An uninterned symbol.
    Uninterned,
}

/// The decoded components of a symbol token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolToken {
    /// Token namespace.
    pub kind: SymbolTokenKind,
    /// Optional package qualifier.
    pub package: Option<String>,
    /// Symbol name.
    pub name: String,
    /// Whether the qualifier was external.
    pub external: bool,
    /// Whether any character was escaped.
    pub escaped: bool,
}

/// Errors returned while parsing a symbol token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolTokenError {
    /// An escape sequence did not terminate.
    UnterminatedEscape,
    /// The decoded symbol has no name.
    EmptyName,
    /// The package qualifier is malformed.
    InvalidQualifier,
}

impl fmt::Display for SymbolTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedEscape => write!(formatter, "unterminated symbol escape"),
            Self::EmptyName => write!(formatter, "symbol name is empty"),
            Self::InvalidQualifier => write!(formatter, "invalid symbol package qualifier"),
        }
    }
}

impl Error for SymbolTokenError {}

#[derive(Clone, Debug)]
struct DecodedChar {
    value: char,
    escaped: bool,
}

/// Parses one symbol token into its namespace and name components.
///
/// # Errors
///
/// Returns [`SymbolTokenError`] when escaping or package qualification is invalid.
pub fn parse_symbol_token(token: &str) -> Result<SymbolToken, SymbolTokenError> {
    let decoded = decode(token)?;
    if decoded.is_empty() {
        return Err(SymbolTokenError::EmptyName);
    }

    let escaped = decoded.iter().any(|character| character.escaped);
    let separators = decoded
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            (character.value == ':' && !character.escaped).then_some(index)
        })
        .collect::<Vec<_>>();

    if decoded.len() >= 2
        && decoded[0].value == '#'
        && !decoded[0].escaped
        && decoded[1].value == ':'
        && !decoded[1].escaped
    {
        if separators.len() != 1 {
            return Err(SymbolTokenError::InvalidQualifier);
        }

        let name = chars_to_string(&decoded[2..]);
        if name.is_empty() {
            return Err(SymbolTokenError::EmptyName);
        }

        return Ok(SymbolToken {
            kind: SymbolTokenKind::Uninterned,
            package: None,
            name,
            external: false,
            escaped,
        });
    }

    match separators.as_slice() {
        [] => Ok(SymbolToken {
            kind: SymbolTokenKind::Symbol,
            package: None,
            name: chars_to_string(&decoded),
            external: false,
            escaped,
        }),
        [separator] => {
            let name = chars_to_string(&decoded[separator + 1..]);
            if name.is_empty() {
                return Err(SymbolTokenError::EmptyName);
            }

            if *separator == 0 {
                return Ok(SymbolToken {
                    kind: SymbolTokenKind::Keyword,
                    package: None,
                    name,
                    external: true,
                    escaped,
                });
            }

            let package = chars_to_string(&decoded[..*separator]);
            Ok(SymbolToken {
                kind: SymbolTokenKind::Symbol,
                package: Some(package),
                name,
                external: true,
                escaped,
            })
        }
        [first, second] if *second == *first + 1 => {
            let package = chars_to_string(&decoded[..*first]);
            let name = chars_to_string(&decoded[*second + 1..]);
            if package.is_empty() || name.is_empty() {
                return Err(SymbolTokenError::InvalidQualifier);
            }

            Ok(SymbolToken {
                kind: SymbolTokenKind::Symbol,
                package: Some(package),
                name,
                external: false,
                escaped,
            })
        }
        _ => Err(SymbolTokenError::InvalidQualifier),
    }
}

fn decode(token: &str) -> Result<Vec<DecodedChar>, SymbolTokenError> {
    let mut decoded = Vec::new();
    let mut characters = token.chars();
    let mut in_vertical_bars = false;

    while let Some(character) = characters.next() {
        if character == '\\' {
            let Some(escaped) = characters.next() else {
                return Err(SymbolTokenError::UnterminatedEscape);
            };
            decoded.push(DecodedChar {
                value: escaped,
                escaped: true,
            });
            continue;
        }

        if character == '|' {
            in_vertical_bars = !in_vertical_bars;
            continue;
        }

        decoded.push(DecodedChar {
            value: if in_vertical_bars {
                character
            } else {
                character.to_ascii_uppercase()
            },
            escaped: in_vertical_bars,
        });
    }

    if in_vertical_bars {
        return Err(SymbolTokenError::UnterminatedEscape);
    }

    Ok(decoded)
}

fn chars_to_string(characters: &[DecodedChar]) -> String {
    characters.iter().map(|character| character.value).collect()
}

#[cfg(test)]
mod tests {
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
            let token =
                parse_symbol_token(input).unwrap_or_else(|error| panic!("{input}: {error}"));
            assert_eq!(token.kind, kind, "{input}");
            assert_eq!(token.package.as_deref(), package, "{input}");
            assert_eq!(token.name, name, "{input}");
            assert_eq!(token.external, external, "{input}");
            assert_eq!(token.escaped, escaped, "{input}");
        }
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
}

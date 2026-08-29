use super::decode::{chars_to_string, decode};
use super::{SymbolToken, SymbolTokenError, SymbolTokenKind};

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

use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolTokenKind {
    Symbol,
    Keyword,
    Uninterned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolToken {
    pub kind: SymbolTokenKind,
    pub package: Option<String>,
    pub name: String,
    pub external: bool,
    pub escaped: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolTokenError {
    UnterminatedEscape,
    EmptyName,
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
            if package.is_empty() {
                return Err(SymbolTokenError::InvalidQualifier);
            }

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

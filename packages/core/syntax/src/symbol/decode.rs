use super::SymbolTokenError;

#[derive(Clone, Debug)]
pub(super) struct DecodedChar {
    pub(super) value: char,
    pub(super) escaped: bool,
}

pub(super) fn decode(token: &str) -> Result<Vec<DecodedChar>, SymbolTokenError> {
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

pub(super) fn chars_to_string(characters: &[DecodedChar]) -> String {
    characters.iter().map(|character| character.value).collect()
}

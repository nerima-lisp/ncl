use super::{RuntimeError, Value, character_argument, exact, string_designator};

pub fn character_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-name", 1)?;
    Ok(
        named_character_name(character_argument("char-name", &arguments[0])?)
            .map_or(Value::Nil, Value::string),
    )
}

pub fn name_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "name-char", 1)?;
    let name = string_designator("name-char", &arguments[0])?;
    if let Some(character) = named_character_from_name(&name) {
        return Ok(Value::Character(character));
    }
    let mut characters = name.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Ok(Value::Character(character)),
        _ => Ok(Value::Nil),
    }
}

const fn named_character_name(character: char) -> Option<&'static str> {
    match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        '\x7f' => Some("Rubout"),
        _ => None,
    }
}

fn named_character_from_name(name: &str) -> Option<char> {
    match name.to_ascii_uppercase().as_str() {
        "NULL" | "NUL" => Some('\0'),
        "BELL" => Some('\x07'),
        "BACKSPACE" | "BS" => Some('\x08'),
        "TAB" => Some('\t'),
        "NEWLINE" | "LINEFEED" | "LF" => Some('\n'),
        "PAGE" | "FORMFEED" | "FF" => Some('\x0c'),
        "RETURN" | "CR" => Some('\r'),
        "SPACE" => Some(' '),
        "RUBOUT" | "DELETE" | "DEL" => Some('\x7f'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_string(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn names_and_resolves_named_characters() {
        assert_eq!(
            ok_string(character_name(&[Value::Character('\n')])),
            Value::string("Newline").to_string()
        );
        assert_eq!(
            ok_string(name_character(&[Value::string("lf")])),
            Value::Character('\n').to_string()
        );
        assert_eq!(
            ok_string(name_character(&[Value::string("λ")])),
            Value::Character('λ').to_string()
        );
        assert_eq!(
            ok_string(name_character(&[Value::string("unknown")])),
            Value::Nil.to_string()
        );
        let named = [
            ('\0', "Null", &["null", "nul"] as &[&str]),
            ('\x07', "Bell", &["bell"]),
            ('\x08', "Backspace", &["backspace", "bs"]),
            ('\t', "Tab", &["tab"]),
            ('\n', "Newline", &["newline", "linefeed", "lf"]),
            ('\x0c', "Page", &["page", "formfeed", "ff"]),
            ('\r', "Return", &["return", "cr"]),
            (' ', "Space", &["space"]),
            ('\x7f', "Rubout", &["rubout", "delete", "del"]),
        ];
        for (character, name, aliases) in named {
            assert_eq!(named_character_name(character), Some(name));
            for alias in aliases {
                assert_eq!(named_character_from_name(alias), Some(character));
            }
        }
        assert_eq!(named_character_name('x'), None);
        assert_eq!(named_character_from_name("unknown"), None);
    }
}

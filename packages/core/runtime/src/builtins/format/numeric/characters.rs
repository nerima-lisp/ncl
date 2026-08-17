fn format_character_directive(
    character: char,
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> String {
    let name = match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        _ => None,
    };
    if at_sign_modifier {
        let mut result = String::from("#\\");
        if let Some(name) = name {
            result.push_str(name);
        } else {
            result.push(character);
        }
        result
    } else if colon_modifier {
        name.map(str::to_string)
            .unwrap_or_else(|| character.to_string())
    } else {
        character.to_string()
    }
}

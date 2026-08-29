use super::{character_argument, exact, sequence_elements, string_designator};
use crate::{RuntimeError, Value};

pub fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

pub fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

pub fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

pub fn trim_string(
    arguments: &[Value],
    function: &str,
    trim_left: bool,
    trim_right: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let trim_set = sequence_elements(function, &arguments[0])?
        .into_iter()
        .map(|value| character_argument(function, &value))
        .collect::<Result<Vec<_>, _>>()?;
    let value = string_designator(function, &arguments[1])?;
    let characters = value.chars().collect::<Vec<_>>();
    let is_trimmed = |character: &char| trim_set.contains(character);
    let start = if trim_left {
        characters
            .iter()
            .position(|character| !is_trimmed(character))
    } else {
        Some(0)
    }
    .unwrap_or(characters.len());
    let end = if trim_right {
        characters
            .iter()
            .rposition(|character| !is_trimmed(character))
            .map_or(0, |index| index + 1)
    } else {
        characters.len()
    };
    Ok(Value::string(
        characters[start.min(end)..end].iter().collect::<String>(),
    ))
}

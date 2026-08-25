use super::{
    arity, integer_argument, number_argument, printed_value, stream_state_error, type_error,
};
use crate::{RuntimeError, Value};
mod numbers;
use numbers::{
    format_dollar_float_directive, format_exponential_float_directive,
    format_fixed_float_directive, format_general_float_directive, format_integer_directive,
};

pub(super) fn format_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("format", "at least 2", arguments.len()));
    }
    let control = match &arguments[1] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("format", "a string control", value)),
    };
    let at_line_start = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().at_line_start(),
        _ => None,
    };
    let output =
        format_control_with_line_state(control, &arguments[2..], at_line_start.unwrap_or(true))?;
    match &arguments[0] {
        Value::Nil => Ok(Value::string(output)),
        Value::Boolean(true) => {
            print!("{output}");
            Ok(Value::Nil)
        }
        Value::Stream(stream) => {
            if stream.borrow_mut().write(&output) {
                Ok(Value::Nil)
            } else {
                Err(stream_state_error("format", "an open output stream"))
            }
        }
        value => Err(type_error("format", "NIL or T as the destination", value)),
    }
}
pub(crate) fn format_control(control: &str, arguments: &[Value]) -> Result<String, RuntimeError> {
    format_control_with_line_state(control, arguments, true)
}

fn format_control_with_line_state(
    control: &str,
    arguments: &[Value],
    at_line_start: bool,
) -> Result<String, RuntimeError> {
    let characters = control.chars().collect::<Vec<_>>();
    let (output, _, _) = format_control_characters(&characters, arguments, false, at_line_start)?;
    Ok(output)
}

#[derive(Clone, Copy)]
enum FormatParameter {
    Missing,
    Number(i64),
    Character(char),
}

#[derive(Clone, Copy)]
struct FormatTermination {
    colon_modifier: bool,
}

fn current_at_line_start(output: &str, initial_at_line_start: bool) -> bool {
    output
        .chars()
        .last()
        .map_or(initial_at_line_start, |character| character == '\n')
}

fn parse_format_parameters(
    characters: &[char],
    character_index: &mut usize,
    arguments: &[Value],
    argument_index: &mut usize,
) -> Result<Vec<FormatParameter>, RuntimeError> {
    let mut parameters = Vec::new();
    let mut current_parameter = None;
    let mut comma_seen = false;
    while *character_index < characters.len() {
        match characters[*character_index] {
            ',' => {
                parameters.push(current_parameter.take().unwrap_or(FormatParameter::Missing));
                comma_seen = true;
                *character_index += 1;
            }
            '\'' => {
                *character_index += 1;
                let character = characters.get(*character_index).copied().ok_or_else(|| {
                    RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    }
                })?;
                current_parameter = Some(FormatParameter::Character(character));
                comma_seen = false;
                *character_index += 1;
            }
            '#' => {
                *character_index += 1;
                let remaining = arguments.len().saturating_sub(*argument_index);
                let remaining = i64::try_from(remaining).unwrap_or(i64::MAX);
                current_parameter = Some(FormatParameter::Number(remaining));
                comma_seen = false;
            }
            'v' | 'V' => {
                *character_index += 1;
                let argument = format_argument("format parameter", arguments, argument_index)?;
                current_parameter = Some(FormatParameter::Number(integer_argument(
                    "format parameter",
                    argument,
                )?));
                comma_seen = false;
            }
            '-' | '0'..='9' => {
                let start = *character_index;
                if characters[*character_index] == '-' {
                    *character_index += 1;
                }
                let digit_start = *character_index;
                while *character_index < characters.len()
                    && characters[*character_index].is_ascii_digit()
                {
                    *character_index += 1;
                }
                if digit_start == *character_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
                let text = characters[start..*character_index]
                    .iter()
                    .collect::<String>();
                let value = text.parse::<i64>().map_err(|_| RuntimeError::InvalidForm {
                    message: format!("format numeric parameter is out of range: {text}"),
                    span: None,
                })?;
                current_parameter = Some(FormatParameter::Number(value));
                comma_seen = false;
            }
            _ => break,
        }
    }
    if let Some(parameter) = current_parameter {
        parameters.push(parameter);
    } else if comma_seen {
        parameters.push(FormatParameter::Missing);
    }
    Ok(parameters)
}

fn format_directive_prefix(
    characters: &[char],
    start: usize,
) -> Result<(usize, bool, bool), RuntimeError> {
    let mut directive_index = start;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ',' | '#' | 'v' | 'V' => directive_index += 1,
            '\'' => {
                directive_index += 1;
                if directive_index >= characters.len() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    });
                }
                directive_index += 1;
            }
            '-' | '0'..='9' => {
                if characters[directive_index] == '-' {
                    directive_index += 1;
                }
                let digit_start = directive_index;
                while directive_index < characters.len()
                    && characters[directive_index].is_ascii_digit()
                {
                    directive_index += 1;
                }
                if digit_start == directive_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
            }
            _ => break,
        }
    }
    let mut colon_modifier = false;
    let mut at_sign_modifier = false;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ':' => {
                colon_modifier = true;
                directive_index += 1;
            }
            '@' => {
                at_sign_modifier = true;
                directive_index += 1;
            }
            _ => break,
        }
    }
    Ok((directive_index, colon_modifier, at_sign_modifier))
}

fn format_control_characters(
    characters: &[char],
    arguments: &[Value],
    colon_iteration_last: bool,
    initial_at_line_start: bool,
) -> Result<(String, usize, Option<FormatTermination>), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut character_index = 0;
    while character_index < characters.len() {
        let character = characters[character_index];
        character_index += 1;
        if character != '~' {
            output.push(character);
            continue;
        }

        let parameters = parse_format_parameters(
            characters,
            &mut character_index,
            arguments,
            &mut argument_index,
        )?;
        let mut colon_modifier = false;
        let mut at_sign_modifier = false;
        while character_index < characters.len() {
            match characters[character_index] {
                ':' => {
                    colon_modifier = true;
                    character_index += 1;
                }
                '@' => {
                    at_sign_modifier = true;
                    character_index += 1;
                }
                _ => break,
            }
        }
        let directive =
            characters
                .get(character_index)
                .copied()
                .ok_or_else(|| RuntimeError::InvalidForm {
                    message: "format control ends after a tilde".to_string(),
                    span: None,
                })?;
        character_index += 1;
        if parameters.is_empty() && !colon_modifier && !at_sign_modifier {
            match directive {
                '\n' => {
                    while characters
                        .get(character_index)
                        .is_some_and(|character| character.is_whitespace())
                    {
                        character_index += 1;
                    }
                    continue;
                }
                '\r' if characters.get(character_index) == Some(&'\n') => {
                    character_index += 1;
                    while characters
                        .get(character_index)
                        .is_some_and(|character| character.is_whitespace())
                    {
                        character_index += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }
        let directive = directive.to_ascii_uppercase();
        let supports_modifiers = matches!(
            directive,
            '{' | '['
                | '<'
                | 'A'
                | 'S'
                | 'C'
                | 'D'
                | 'B'
                | 'O'
                | 'X'
                | 'R'
                | 'F'
                | 'E'
                | 'G'
                | 'I'
                | 'P'
                | '$'
                | '^'
                | 'T'
                | 'W'
                | '?'
                | '*'
                | '_'
                | '('
        );
        if (colon_modifier || at_sign_modifier) && !supports_modifiers {
            return Err(RuntimeError::InvalidForm {
                message: format!("unsupported format modifier before ~{directive}"),
                span: None,
            });
        }
        match directive {
            'A' => {
                let argument = format_argument("~A", arguments, &mut argument_index)?;
                let mut formatted = String::new();
                if colon_modifier && matches!(argument, Value::Nil) {
                    formatted.push_str("()");
                } else {
                    append_aesthetic(&mut formatted, argument);
                }
                output.push_str(&format_text_field(
                    formatted,
                    &parameters,
                    at_sign_modifier,
                )?);
            }
            'S' => {
                let argument = format_argument("~S", arguments, &mut argument_index)?;
                let formatted = if colon_modifier && matches!(argument, Value::Nil) {
                    "()".to_string()
                } else {
                    argument.to_string()
                };
                output.push_str(&format_text_field(
                    formatted,
                    &parameters,
                    at_sign_modifier,
                )?);
            }
            '(' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format case conversion does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let body_end = format_case_conversion_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let (formatted, consumed, termination) = format_control_characters(
                    body,
                    &arguments[argument_index..],
                    colon_iteration_last,
                    current_at_line_start(&output, initial_at_line_start),
                )?;
                output.push_str(&format_case_conversion(
                    &formatted,
                    colon_modifier,
                    at_sign_modifier,
                ));
                argument_index += consumed;
                if let Some(termination) = termination {
                    return Ok((output, argument_index, Some(termination)));
                }
            }
            'D' | 'B' | 'O' | 'X' => {
                let argument =
                    format_argument("format integer directive", arguments, &mut argument_index)?;
                let integer = integer_argument("format", argument)?;
                let radix = match directive {
                    'D' => 10,
                    'B' => 2,
                    'O' => 8,
                    'X' => 16,
                    _ => unreachable!(),
                };
                output.push_str(&format_integer_directive(
                    integer,
                    radix,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'F' => {
                let argument = format_argument("~F", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_fixed_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'G' => {
                let argument = format_argument("~G", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_general_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'E' => {
                let argument = format_argument("~E", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_exponential_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            '$' => {
                let argument = format_argument("~$", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_dollar_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'P' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~P does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let argument = if colon_modifier {
                    let index =
                        argument_index
                            .checked_sub(1)
                            .ok_or_else(|| RuntimeError::InvalidForm {
                                message: "format ~:P has no previous argument".to_string(),
                                span: None,
                            })?;
                    arguments
                        .get(index)
                        .ok_or_else(|| RuntimeError::InvalidForm {
                            message: "format ~:P has no previous argument".to_string(),
                            span: None,
                        })?
                } else {
                    format_argument("~P", arguments, &mut argument_index)?
                };
                let value = integer_argument("format", argument)?;
                if at_sign_modifier {
                    output.push_str(if value == 1 { "y" } else { "ies" });
                } else if value == 1 {
                    output.push_str("");
                } else {
                    output.push('s');
                }
            }
            'C' => {
                let argument = format_argument("~C", arguments, &mut argument_index)?;
                let Value::Character(character) = argument else {
                    return Err(type_error("format", "a character for ~C", argument));
                };
                output.push_str(&format_character_directive(
                    *character,
                    colon_modifier,
                    at_sign_modifier,
                ));
            }
            '%' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for _ in 0..count {
                    output.push('\n');
                }
            }
            '&' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for repetition in 0..count {
                    if repetition == 0 {
                        if (!output.is_empty() && !output.ends_with('\n'))
                            || (output.is_empty() && !initial_at_line_start)
                        {
                            output.push('\n');
                        }
                    } else {
                        output.push('\n');
                    }
                }
            }
            '|' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for _ in 0..count {
                    output.push('\x0c');
                }
            }
            '~' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for _ in 0..count {
                    output.push('~');
                }
            }
            '_' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~_ does not accept parameters".to_string(),
                        span: None,
                    });
                }
            }
            'I' => {
                if at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~I does not support the at-sign modifier".to_string(),
                        span: None,
                    });
                }
                if parameters.len() > 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~I accepts at most one parameter".to_string(),
                        span: None,
                    });
                }
                let _ = format_parameter_number(&parameters, 0, 0)?;
            }
            '*' => {
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~* cannot combine the colon and at-sign modifiers"
                            .to_string(),
                        span: None,
                    });
                }
                let default = if at_sign_modifier { 0 } else { 1 };
                let count = format_parameter_count(&parameters, 0, default)?;
                if colon_modifier {
                    argument_index = argument_index.checked_sub(count).ok_or_else(|| {
                        RuntimeError::InvalidForm {
                            message: "format ~:* moved before the first argument".to_string(),
                            span: None,
                        }
                    })?;
                } else if at_sign_modifier {
                    argument_index = count;
                } else {
                    argument_index = argument_index.saturating_add(count).min(arguments.len());
                }
            }
            '?' => {
                if !parameters.is_empty() || colon_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~? only supports the at-sign modifier".to_string(),
                        span: None,
                    });
                }
                let nested_control = format_argument("~?", arguments, &mut argument_index)?;
                let nested_control = match nested_control {
                    Value::String(value) => value,
                    value => return Err(type_error("format", "a string for ~?", value)),
                };
                if at_sign_modifier {
                    let nested_characters = nested_control.chars().collect::<Vec<_>>();
                    let (formatted, consumed, termination) = format_control_characters(
                        &nested_characters,
                        &arguments[argument_index..],
                        false,
                        current_at_line_start(&output, initial_at_line_start),
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
                    }
                } else {
                    let nested_arguments = format_argument("~?", arguments, &mut argument_index)?;
                    let nested_arguments = nested_arguments.list_items().ok_or_else(|| {
                        type_error("format", "a list of arguments for ~?", nested_arguments)
                    })?;
                    output.push_str(&format_control_with_line_state(
                        &nested_control,
                        &nested_arguments,
                        current_at_line_start(&output, initial_at_line_start),
                    )?);
                }
            }
            '^' => {
                if format_escape_upward(
                    &parameters,
                    arguments,
                    argument_index,
                    colon_modifier,
                    colon_iteration_last,
                )? {
                    return Ok((
                        output,
                        argument_index,
                        Some(FormatTermination { colon_modifier }),
                    ));
                }
            }
            '{' => {
                let body_end = format_iteration_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let limit = format_iteration_limit(&parameters)?;
                if at_sign_modifier {
                    let (formatted, consumed) = format_iteration(
                        body,
                        &arguments[argument_index..],
                        colon_modifier,
                        limit,
                        current_at_line_start(&output, initial_at_line_start),
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                } else {
                    let list = format_argument("~{", arguments, &mut argument_index)?;
                    let list = list
                        .list_items()
                        .ok_or_else(|| type_error("format", "a list for ~{", list))?;
                    let (formatted, _) = format_iteration(
                        body,
                        &list,
                        colon_modifier,
                        limit,
                        current_at_line_start(&output, initial_at_line_start),
                    )?;
                    output.push_str(&formatted);
                }
            }
            '[' => {
                let body_end = format_choice_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let clauses = format_choice_clauses(body)?;
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice cannot use both : and @ modifiers".to_string(),
                        span: None,
                    });
                }
                if colon_modifier && clauses.len() != 2 {
                    return Err(RuntimeError::InvalidForm {
                        message: "boolean format choice needs two clauses".to_string(),
                        span: None,
                    });
                }
                if at_sign_modifier && clauses.len() != 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign format choice needs one clause".to_string(),
                        span: None,
                    });
                }
                if (colon_modifier || at_sign_modifier) && !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice parameters cannot be used with : or @ modifier"
                            .to_string(),
                        span: None,
                    });
                }
                if !colon_modifier && !at_sign_modifier && parameters.len() > 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice accepts at most one parameter".to_string(),
                        span: None,
                    });
                }

                let selected_index = if colon_modifier {
                    let selector = format_argument("~[", arguments, &mut argument_index)?;
                    Some(usize::from(selector.is_truthy()))
                } else if at_sign_modifier {
                    let selector =
                        arguments
                            .get(argument_index)
                            .ok_or_else(|| RuntimeError::InvalidForm {
                                message: "format directive ~[ needs another argument".to_string(),
                                span: None,
                            })?;
                    if selector.is_truthy() {
                        Some(0)
                    } else {
                        argument_index += 1;
                        None
                    }
                } else {
                    let has_selector_parameter = matches!(
                        parameters.first().copied(),
                        Some(FormatParameter::Number(_)) | Some(FormatParameter::Character(_))
                    );
                    let index = if has_selector_parameter {
                        format_parameter_number(&parameters, 0, 0)?
                    } else {
                        let selector = format_argument("~[", arguments, &mut argument_index)?;
                        integer_argument("format choice", selector)?
                    };
                    usize::try_from(index).ok()
                };
                let selected_clause = selected_index.and_then(|index| {
                    clauses
                        .get(index)
                        .or_else(|| clauses.iter().find(|(_, default)| *default))
                });
                if let Some((clause, _)) = selected_clause {
                    let (formatted, consumed, termination) = format_control_characters(
                        clause,
                        &arguments[argument_index..],
                        colon_iteration_last,
                        current_at_line_start(&output, initial_at_line_start),
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
                    }
                } else if !colon_modifier && !at_sign_modifier {
                    if let Some((clause, _)) = clauses.iter().find(|(_, default)| *default) {
                        let (formatted, consumed, termination) = format_control_characters(
                            clause,
                            &arguments[argument_index..],
                            colon_iteration_last,
                            current_at_line_start(&output, initial_at_line_start),
                        )?;
                        output.push_str(&formatted);
                        argument_index += consumed;
                        if let Some(termination) = termination {
                            return Ok((output, argument_index, Some(termination)));
                        }
                    }
                }
            }
            '<' => {
                if parameters.len() > 4 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format justification accepts at most four parameters".to_string(),
                        span: None,
                    });
                }
                let body_end = format_justification_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let clauses = format_justification_clauses(body)?;
                let (formatted, consumed) = format_justification(
                    &clauses,
                    &arguments[argument_index..],
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                    colon_iteration_last,
                    current_at_line_start(&output, initial_at_line_start),
                )?;
                output.push_str(&formatted);
                argument_index += consumed;
            }
            'R' => {
                let argument = format_argument("~R", arguments, &mut argument_index)?;
                let integer = integer_argument("format", argument)?;
                output.push_str(&format_radix_directive(
                    integer,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'T' => {
                let column = format_parameter_count(&parameters, 0, 1)?;
                let increment = format_parameter_count(&parameters, 1, 1)?;
                if !colon_modifier {
                    let current_column = output
                        .rsplit('\n')
                        .next()
                        .unwrap_or_default()
                        .chars()
                        .count();
                    let spaces = if at_sign_modifier {
                        let relative_column = current_column.saturating_add(column);
                        let additional = if increment == 0 {
                            0
                        } else {
                            (increment - (relative_column % increment)) % increment
                        };
                        column.saturating_add(additional)
                    } else if current_column < column {
                        column - current_column
                    } else if increment == 0 {
                        0
                    } else {
                        increment - ((current_column - column) % increment)
                    };
                    output.extend(std::iter::repeat(' ').take(spaces));
                }
            }
            'W' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~W does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let argument = format_argument("~W", arguments, &mut argument_index)?;
                output.push_str(&printed_value(argument, true));
            }
            '}' => {
                return Err(RuntimeError::InvalidForm {
                    message: "unexpected format iteration terminator ~}".to_string(),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unsupported format directive ~{directive}"),
                    span: None,
                });
            }
        }
    }
    Ok((output, argument_index, None))
}

fn format_iteration_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '{', "format iteration is missing ~}")
}

fn format_choice_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '[', "format choice is missing ~]")
}

fn format_justification_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '<', "format justification is missing ~>")
}

fn format_case_conversion_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(
        characters,
        start,
        '(',
        "format case conversion is missing ~)",
    )
}

fn format_directive_end(
    characters: &[char],
    start: usize,
    opening: char,
    missing_message: &str,
) -> Result<usize, RuntimeError> {
    let mut stack = vec![opening];
    let mut index = start;
    while index < characters.len() {
        if characters[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, _, _) = format_directive_prefix(characters, index + 1)?;
        let Some(directive) = characters.get(directive_index).copied() else {
            break;
        };
        match directive.to_ascii_uppercase() {
            '{' | '[' | '<' | '(' => stack.push(directive.to_ascii_uppercase()),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                    if stack.is_empty() {
                        return Ok(index);
                    }
                }
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    Err(RuntimeError::InvalidForm {
        message: missing_message.to_string(),
        span: None,
    })
}

fn format_choice_clauses<'a>(body: &'a [char]) -> Result<Vec<(&'a [char], bool)>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut default_clause = false;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, colon_modifier, at_sign_modifier) =
            format_directive_prefix(body, index + 1)?;
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format choice clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' | '<' | '(' => stack.push(directive),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format choice terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign modifier is not supported on a format choice clause"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push((&body[clause_start..index], default_clause));
                clause_start = directive_index + 1;
                default_clause = colon_modifier;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format choice contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push((&body[clause_start..], default_clause));
    Ok(clauses)
}

fn format_justification_clauses<'a>(body: &'a [char]) -> Result<Vec<&'a [char]>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, colon_modifier, at_sign_modifier) =
            format_directive_prefix(body, index + 1)?;
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format justification clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' | '<' | '(' => stack.push(directive),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format justification terminator ~{directive}"),
                        span: None,
                    });
                } else {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("mismatched format justification terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if colon_modifier || at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format justification does not support modifiers on ~;"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push(&body[clause_start..index]);
                clause_start = directive_index + 1;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format justification contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push(&body[clause_start..]);
    Ok(clauses)
}

fn format_justification(
    clauses: &[&[char]],
    arguments: &[Value],
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
    colon_iteration_last: bool,
    initial_at_line_start: bool,
) -> Result<(String, usize), RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let column_increment = format_parameter_count(parameters, 1, 1)?;
    let minimum_padding = format_parameter_count(parameters, 2, 0)?;
    let pad_character = format_parameter_character(parameters, 3, ' ')?;
    if column_increment == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format justification column increment must be positive".to_string(),
            span: None,
        });
    }

    let mut pieces = Vec::new();
    let mut argument_index = 0;
    for clause in clauses {
        let clause_at_line_start = if pieces.is_empty() {
            initial_at_line_start
        } else {
            false
        };
        let (formatted, consumed, termination) = format_control_characters(
            clause,
            &arguments[argument_index..],
            colon_iteration_last,
            clause_at_line_start,
        )?;
        argument_index += consumed;
        if termination.is_some() {
            break;
        }
        pieces.push(formatted);
    }

    if pieces.is_empty() {
        return Ok((String::new(), argument_index));
    }

    let between_count = pieces.len().saturating_sub(1);
    let content_width = pieces.iter().fold(0usize, |width, piece| {
        width.saturating_add(piece.chars().count())
    });
    let required_width =
        content_width.saturating_add(minimum_padding.saturating_mul(between_count));
    let mut target_width = minimum_column.max(required_width);
    if target_width > minimum_column {
        let remainder = (target_width - minimum_column) % column_increment;
        if remainder != 0 {
            target_width = target_width.saturating_add(column_increment - remainder);
        }
    }
    let total_padding = target_width.saturating_sub(content_width);
    let base_between_padding = minimum_padding.saturating_mul(between_count);

    let leading_gap = if pieces.len() == 1 {
        colon_modifier || !at_sign_modifier
    } else {
        colon_modifier
    };
    let trailing_gap = at_sign_modifier;
    let gap_count = (if leading_gap { 1usize } else { 0usize })
        .saturating_add(between_count)
        .saturating_add(if trailing_gap { 1usize } else { 0usize });
    let distributed_padding = total_padding.saturating_sub(base_between_padding);
    let base_padding = if gap_count == 0 {
        0
    } else {
        distributed_padding / gap_count
    };
    let remainder = if gap_count == 0 {
        0
    } else {
        distributed_padding % gap_count
    };
    let mut gaps = vec![0usize; gap_count];
    for (index, gap) in gaps.iter_mut().enumerate() {
        *gap = base_padding.saturating_add(usize::from(index >= gap_count - remainder));
    }

    let mut gap_index = 0;
    if leading_gap {
        gap_index += 1;
    }
    for _ in 0..between_count {
        gaps[gap_index] = gaps[gap_index].saturating_add(minimum_padding);
        gap_index += 1;
    }

    let mut output = String::new();
    let append_padding = |output: &mut String, count: usize| {
        output.extend(std::iter::repeat(pad_character).take(count));
    };
    gap_index = 0;
    if leading_gap {
        append_padding(&mut output, gaps[gap_index]);
        gap_index += 1;
    }
    for (index, piece) in pieces.iter().enumerate() {
        output.push_str(piece);
        if index + 1 < pieces.len() {
            append_padding(&mut output, gaps[gap_index]);
            gap_index += 1;
        }
    }
    if trailing_gap {
        append_padding(&mut output, gaps[gap_index]);
    }
    Ok((output, argument_index))
}

fn format_case_conversion(text: &str, colon_modifier: bool, at_sign_modifier: bool) -> String {
    let mut output = String::new();
    if colon_modifier && at_sign_modifier {
        for character in text.chars() {
            output.extend(character.to_uppercase());
        }
        return output;
    }

    if colon_modifier {
        let mut word_start = true;
        for character in text.chars() {
            if character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.push(character);
                word_start = true;
            }
        }
        return output;
    }

    if at_sign_modifier {
        let mut first_word = true;
        let mut word_start = true;
        for character in text.chars() {
            if first_word && character.is_whitespace() {
                first_word = false;
                output.push(character);
            } else if first_word && character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.extend(character.to_lowercase());
            }
        }
        return output;
    }

    for character in text.chars() {
        output.extend(character.to_lowercase());
    }
    output
}

fn format_escape_upward(
    parameters: &[FormatParameter],
    arguments: &[Value],
    argument_index: usize,
    colon_modifier: bool,
    colon_iteration_last: bool,
) -> Result<bool, RuntimeError> {
    if parameters.is_empty() {
        return Ok(if colon_modifier {
            colon_iteration_last
        } else {
            argument_index >= arguments.len()
        });
    }
    if parameters.len() > 3 {
        return Err(RuntimeError::InvalidForm {
            message: "format ~^ accepts at most three parameters".to_string(),
            span: None,
        });
    }
    let values = parameters
        .iter()
        .map(|parameter| match parameter {
            FormatParameter::Missing => Ok(0),
            FormatParameter::Number(value) => Ok(*value),
            FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
                message: "format ~^ parameters must be numeric".to_string(),
                span: None,
            }),
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    Ok(match values.as_slice() {
        [value] => *value == 0,
        [first, second] => first == second,
        [first, second, third] => first <= second && second <= third,
        _ => unreachable!("format ~^ parameter count was checked"),
    })
}

fn format_iteration(
    body: &[char],
    arguments: &[Value],
    colon_modifier: bool,
    limit: Option<usize>,
    initial_at_line_start: bool,
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut repetitions = 0;
    while argument_index < arguments.len() && limit.map_or(true, |limit| repetitions < limit) {
        let (consumed, termination) = if colon_modifier {
            let nested_arguments = arguments[argument_index].list_items().ok_or_else(|| {
                type_error(
                    "format",
                    "a list element for ~:{",
                    &arguments[argument_index],
                )
            })?;
            let (formatted, consumed, termination) = format_control_characters(
                body,
                &nested_arguments,
                argument_index + 1 >= arguments.len(),
                current_at_line_start(&output, initial_at_line_start),
            )?;
            output.push_str(&formatted);
            (consumed, termination)
        } else {
            let (formatted, consumed, termination) = format_control_characters(
                body,
                &arguments[argument_index..],
                false,
                current_at_line_start(&output, initial_at_line_start),
            )?;
            output.push_str(&formatted);
            (consumed, termination)
        };
        argument_index += if colon_modifier { 1 } else { consumed.max(1) };
        repetitions += 1;
        if let Some(termination) = termination {
            if colon_modifier && !termination.colon_modifier {
                continue;
            }
            break;
        }
    }
    Ok((output, argument_index))
}

fn format_parameter_number(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<i64, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Number(value) => Ok(value),
        FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be numeric"),
            span: None,
        }),
    }
}

fn format_parameter_count(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<usize, RuntimeError> {
    let value = format_parameter_number(parameters, index, default)?;
    usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
        message: format!("format parameter {index} must be non-negative"),
        span: None,
    })
}

fn format_parameter_character(
    parameters: &[FormatParameter],
    index: usize,
    default: char,
) -> Result<char, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Character(value) => Ok(value),
        FormatParameter::Number(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be a character"),
            span: None,
        }),
    }
}

fn format_iteration_limit(parameters: &[FormatParameter]) -> Result<Option<usize>, RuntimeError> {
    if parameters.is_empty() || matches!(parameters[0], FormatParameter::Missing) {
        Ok(None)
    } else {
        Ok(Some(format_parameter_count(parameters, 0, 0)?))
    }
}

fn format_text_field(
    text: String,
    parameters: &[FormatParameter],
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let column_increment = format_parameter_count(parameters, 1, 1)?;
    let minimum_padding = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;
    if column_increment == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format column increment must be positive".to_string(),
            span: None,
        });
    }

    let width = text.chars().count();
    let mut target = minimum_column.max(width.saturating_add(minimum_padding));
    if target > minimum_column {
        let remainder = (target - minimum_column) % column_increment;
        if remainder != 0 {
            target += column_increment - remainder;
        }
    }
    let padding = target.saturating_sub(width);
    let mut formatted = String::new();
    if at_sign_modifier {
        formatted.extend(std::iter::repeat(padding_character).take(padding));
        formatted.push_str(&text);
    } else {
        formatted.push_str(&text);
        formatted.extend(std::iter::repeat(padding_character).take(padding));
    }
    Ok(formatted)
}

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

fn format_radix_directive(
    value: i64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if let Some(parameter) = parameters.first().copied() {
        if !matches!(parameter, FormatParameter::Missing) {
            let radix = match parameter {
                FormatParameter::Number(value) => {
                    u32::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                        message: "format radix must be between 2 and 36".to_string(),
                        span: None,
                    })?
                }
                FormatParameter::Missing => unreachable!(),
                FormatParameter::Character(_) => {
                    return Err(RuntimeError::InvalidForm {
                        message: "format radix must be numeric".to_string(),
                        span: None,
                    });
                }
            };
            if !(2..=36).contains(&radix) {
                return Err(RuntimeError::InvalidForm {
                    message: "format radix must be between 2 and 36".to_string(),
                    span: None,
                });
            }
            return format_integer_directive(
                value,
                radix,
                &parameters[1..],
                false,
                at_sign_modifier,
            );
        }
    }
    if at_sign_modifier {
        format_roman_number(value, colon_modifier)
    } else {
        Ok(format_english_number(value, colon_modifier))
    }
}

fn format_english_number(value: i64, ordinal: bool) -> String {
    if value < 0 {
        if value == i64::MIN {
            return format!(
                "minus {}",
                format_unsigned_integer(value.unsigned_abs(), 10)
            );
        }
        return format!(
            "minus {}",
            format_english_number(value.wrapping_neg(), ordinal)
        );
    }
    let magnitude = value as u64;
    if magnitude == 0 {
        return if ordinal {
            "zeroth".to_string()
        } else {
            "zero".to_string()
        };
    }
    const GROUPS: &[&str] = &[
        "",
        "thousand",
        "million",
        "billion",
        "trillion",
        "quadrillion",
    ];
    let mut chunks = Vec::new();
    let mut remainder = magnitude;
    while remainder != 0 {
        chunks.push(remainder % 1000);
        remainder /= 1000;
    }
    if chunks.len() > GROUPS.len() {
        return format_integer_radix(value, 10);
    }
    let ordinal_group = if ordinal {
        chunks.iter().position(|chunk| *chunk != 0)
    } else {
        None
    };
    let mut parts = Vec::new();
    for index in (0..chunks.len()).rev() {
        let chunk = chunks[index];
        if chunk == 0 {
            continue;
        }
        let group_is_ordinal = ordinal_group == Some(index);
        let mut part = if group_is_ordinal && index == 0 {
            english_under_thousand(chunk, true)
        } else {
            english_under_thousand(chunk, false)
        };
        if index != 0 {
            part.push(' ');
            part.push_str(GROUPS[index]);
            if group_is_ordinal {
                part.push_str("th");
            }
        }
        parts.push(part);
    }
    parts.join(" ")
}

fn english_under_thousand(value: u64, ordinal: bool) -> String {
    const CARDINALS: &[&str] = &[
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const ORDINALS: &[&str] = &[
        "zeroth",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    const TENS: &[&str] = &[
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    const ORDINAL_TENS: &[&str] = &[
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    if value < 20 {
        return if ordinal {
            ORDINALS[value as usize].to_string()
        } else {
            CARDINALS[value as usize].to_string()
        };
    }
    if value < 100 {
        let tens = value / 10;
        let ones = value % 10;
        if ones == 0 {
            return if ordinal {
                ORDINAL_TENS[tens as usize].to_string()
            } else {
                TENS[tens as usize].to_string()
            };
        }
        return format!(
            "{}-{}",
            TENS[tens as usize],
            english_under_thousand(ones, ordinal)
        );
    }
    let hundreds = value / 100;
    let remainder = value % 100;
    if remainder == 0 {
        if ordinal {
            format!("{} hundredth", CARDINALS[hundreds as usize])
        } else {
            format!("{} hundred", CARDINALS[hundreds as usize])
        }
    } else {
        format!(
            "{} hundred {}",
            CARDINALS[hundreds as usize],
            english_under_thousand(remainder, ordinal)
        )
    }
}

fn format_roman_number(value: i64, old_style: bool) -> Result<String, RuntimeError> {
    if value < 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format Roman numerals do not accept negative numbers".to_string(),
            span: None,
        });
    }
    if value == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format Roman numerals do not represent zero".to_string(),
            span: None,
        });
    }
    let magnitude = value as u64;
    if !old_style && magnitude > 3999 {
        return Err(RuntimeError::InvalidForm {
            message: "format modern Roman numerals support values up to 3999".to_string(),
            span: None,
        });
    }
    let numerals: &[(u64, &str)] = if old_style {
        &[
            (1000_u64, "M"),
            (500, "D"),
            (100, "C"),
            (50, "L"),
            (10, "X"),
            (5, "V"),
            (1, "I"),
        ]
    } else {
        &[
            (1000_u64, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ]
    };
    let mut remainder = magnitude;
    let mut result = String::new();
    for &(unit, numeral) in numerals {
        while remainder >= unit {
            result.push_str(numeral);
            remainder -= unit;
        }
    }
    Ok(result)
}

fn format_argument<'a>(
    directive: &str,
    arguments: &'a [Value],
    argument_index: &mut usize,
) -> Result<&'a Value, RuntimeError> {
    let argument = arguments
        .get(*argument_index)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("format directive {directive} needs another argument"),
            span: None,
        })?;
    *argument_index += 1;
    Ok(argument)
}

fn append_aesthetic(output: &mut String, value: &Value) {
    match value {
        Value::String(value) => output.push_str(value),
        Value::Character(value) => output.push(*value),
        Value::List(values) => {
            output.push('(');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        Value::DottedList { items, tail } => {
            output.push('(');
            for (index, value) in items.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            if !items.is_empty() {
                output.push(' ');
            }
            output.push_str(". ");
            append_aesthetic(output, tail);
            output.push(')');
        }
        Value::Vector(values) => {
            output.push_str("#(");
            let values = values.borrow();
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        _ => output.push_str(&value.to_string()),
    }
}

fn format_integer_radix(value: i64, radix: u32) -> String {
    let mut result = format_unsigned_integer(value.unsigned_abs(), radix);
    if value < 0 {
        result.insert(0, '-');
    }
    result
}

fn format_unsigned_integer(mut magnitude: u64, radix: u32) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if magnitude == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while magnitude != 0 {
        digits.push(DIGITS[(magnitude % u64::from(radix)) as usize] as char);
        magnitude /= u64::from(radix);
    }
    digits.iter().rev().collect()
}

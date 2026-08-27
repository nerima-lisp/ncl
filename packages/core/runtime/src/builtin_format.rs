#![allow(clippy::wildcard_imports)]
use super::*;

#[path = "builtin_format_general.rs"]
mod general;
#[allow(clippy::wildcard_imports)]
use general::*;
#[path = "builtin_format_english.rs"]
mod english;
#[allow(clippy::wildcard_imports)]
use english::*;
#[path = "builtin_format_integer_helpers.rs"]
mod integer_helpers;
#[allow(clippy::wildcard_imports)]
use integer_helpers::*;
#[path = "builtin_format_output.rs"]
mod output;
#[allow(clippy::wildcard_imports)]
use output::*;
#[path = "builtin_format_model.rs"]
mod model;
#[allow(clippy::wildcard_imports)]
use model::*;
#[path = "builtin_format_parameters.rs"]
mod parameters;
#[allow(clippy::wildcard_imports)]
use parameters::*;
#[path = "builtin_format_parser.rs"]
mod parser;
#[allow(clippy::wildcard_imports)]
use parser::*;
#[path = "builtin_format_justification.rs"]
mod justification;
#[allow(clippy::wildcard_imports)]
use justification::*;
#[path = "builtin_format_exponential.rs"]
mod exponential;
#[allow(clippy::wildcard_imports)]
use exponential::*;
#[path = "builtin_format_float_helpers.rs"]
mod float_helpers;
#[allow(clippy::wildcard_imports)]
use float_helpers::*;
#[path = "builtin_format_entry.rs"]
mod entry;
pub use entry::format_control;
pub(super) use entry::format_value;

#[path = "builtin_format_boundaries.rs"]
mod boundaries;
#[allow(clippy::wildcard_imports)]
use boundaries::*;

pub(super) fn format_control_characters(
    characters: &[char],
    arguments: &[Value],
    colon_iteration_last: bool,
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
        let FormatDirective {
            parameters,
            directive,
            colon_modifier,
            at_sign_modifier,
        } = parse_format_directive(
            characters,
            &mut character_index,
            arguments,
            &mut argument_index,
        )?;
        if format_simple_directive(
            directive,
            &mut output,
            arguments,
            &mut argument_index,
            &parameters,
            colon_modifier,
            at_sign_modifier,
        )? {
            continue;
        }
        let termination = format_non_simple_directive(
            &mut FormatControlState {
                characters,
                arguments,
                output: &mut output,
                argument_index: &mut argument_index,
                character_index: &mut character_index,
                colon_iteration_last,
            },
            directive,
            &parameters,
            colon_modifier,
            at_sign_modifier,
        )?;
        if let Some(termination) = termination {
            return Ok((output, argument_index, Some(termination)));
        }
    }
    Ok((output, argument_index, None))
}

pub(super) fn format_non_simple_directive(
    state: &mut FormatControlState<'_>,
    directive: char,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    match directive {
        'A' | 'S' => state.output.push_str(&format_value_directive(
            directive,
            state.arguments,
            state.argument_index,
            parameters,
            colon_modifier,
            at_sign_modifier,
        )?),
        '(' => return format_case_directive(state, parameters, colon_modifier, at_sign_modifier),
        'D' | 'B' | 'O' | 'X' | 'F' | 'G' | 'E' | '$' => {
            state.output.push_str(&format_numeric_directive(
                directive,
                state.arguments,
                state.argument_index,
                parameters,
                colon_modifier,
                at_sign_modifier,
            )?);
        }
        '?' | '^' => {
            return format_nested_or_escape_directive(
                directive,
                state,
                parameters,
                colon_modifier,
                at_sign_modifier,
            );
        }
        '{' => format_iteration_directive(state, parameters, colon_modifier, at_sign_modifier)?,
        '[' => return format_choice_directive(state, parameters, colon_modifier, at_sign_modifier),
        '<' => format_justification_directive(state, parameters, colon_modifier, at_sign_modifier)?,
        'R' => format_radix_output(
            state.output,
            state.arguments,
            state.argument_index,
            parameters,
            colon_modifier,
            at_sign_modifier,
        )?,
        'T' => format_tab_output(state.output, parameters, colon_modifier, at_sign_modifier)?,
        'W' => format_write_output(
            state.output,
            state.arguments,
            state.argument_index,
            parameters,
        )?,
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
    Ok(None)
}

pub(super) fn format_justification_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<(), RuntimeError> {
    if parameters.len() > 4 {
        return Err(RuntimeError::InvalidForm {
            message: "format justification accepts at most four parameters".to_string(),
            span: None,
        });
    }
    let body_end = format_justification_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let clauses = format_justification_clauses(body)?;
    let (formatted, consumed) = format_justification(
        &clauses,
        &state.arguments[*state.argument_index..],
        parameters,
        colon_modifier,
        at_sign_modifier,
        state.colon_iteration_last,
    )?;
    state.output.push_str(&formatted);
    *state.argument_index += consumed;
    Ok(())
}

pub(super) fn format_iteration_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<(), RuntimeError> {
    let body_end = format_iteration_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let limit = format_iteration_limit(parameters)?;
    if at_sign_modifier {
        let (formatted, consumed) = format_iteration(
            body,
            &state.arguments[*state.argument_index..],
            colon_modifier,
            limit,
        )?;
        state.output.push_str(&formatted);
        *state.argument_index += consumed;
    } else {
        let list = format_argument("~{", state.arguments, state.argument_index)?;
        let list = list
            .list_items()
            .ok_or_else(|| type_error("format", "a list for ~{", list))?;
        let (formatted, _) = format_iteration(body, &list, colon_modifier, limit)?;
        state.output.push_str(&formatted);
    }
    Ok(())
}

pub(super) fn format_choice_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    let body_end = format_choice_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let clauses = format_choice_clauses(body)?;
    if colon_modifier && (at_sign_modifier || clauses.len() != 2) {
        return Err(RuntimeError::InvalidForm {
            message: "invalid format choice modifiers or clause count".to_string(),
            span: None,
        });
    }
    if at_sign_modifier && clauses.len() != 1 {
        return Err(RuntimeError::InvalidForm {
            message: "at-sign format choice needs one clause".to_string(),
            span: None,
        });
    }
    if (colon_modifier || at_sign_modifier) && !parameters.is_empty()
        || !colon_modifier && !at_sign_modifier && parameters.len() > 1
    {
        return Err(RuntimeError::InvalidForm {
            message: "invalid format choice parameters".to_string(),
            span: None,
        });
    }
    let selected_index = if colon_modifier {
        Some(usize::from(
            format_argument("~[", state.arguments, state.argument_index)?.is_truthy(),
        ))
    } else if at_sign_modifier {
        let selector = state.arguments.get(*state.argument_index).ok_or_else(|| {
            RuntimeError::InvalidForm {
                message: "format directive ~[ needs another argument".to_string(),
                span: None,
            }
        })?;
        if selector.is_truthy() {
            Some(0)
        } else {
            *state.argument_index += 1;
            None
        }
    } else {
        let index = if matches!(
            parameters.first().copied(),
            Some(FormatParameter::Number(_) | FormatParameter::Character(_))
        ) {
            format_parameter_number(parameters, 0, 0)?
        } else {
            integer_argument(
                "format choice",
                format_argument("~[", state.arguments, state.argument_index)?,
            )?
        };
        usize::try_from(index).ok()
    };
    let clause = selected_index
        .and_then(|index| {
            clauses
                .get(index)
                .or_else(|| clauses.iter().find(|(_, default)| *default))
        })
        .or_else(|| {
            (!colon_modifier && !at_sign_modifier)
                .then(|| clauses.iter().find(|(_, default)| *default))
                .flatten()
        });
    if let Some((clause, _)) = clause {
        let (formatted, consumed, termination) = format_control_characters(
            clause,
            &state.arguments[*state.argument_index..],
            state.colon_iteration_last,
        )?;
        state.output.push_str(&formatted);
        *state.argument_index += consumed;
        return Ok(termination);
    }
    Ok(None)
}

pub(super) fn format_case_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    if !parameters.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format case conversion does not accept parameters".to_string(),
            span: None,
        });
    }
    let body_end = format_case_conversion_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let (formatted, consumed, termination) = format_control_characters(
        body,
        &state.arguments[*state.argument_index..],
        state.colon_iteration_last,
    )?;
    state.output.push_str(&format_case_conversion(
        &formatted,
        colon_modifier,
        at_sign_modifier,
    ));
    *state.argument_index += consumed;
    Ok(termination)
}

pub(super) fn format_nested_or_escape_directive(
    directive: char,
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    if directive == '^' {
        if format_escape_upward(
            parameters,
            state.arguments,
            *state.argument_index,
            colon_modifier,
            state.colon_iteration_last,
        )? {
            return Ok(Some(FormatTermination { colon_modifier }));
        }
        return Ok(None);
    }

    if !parameters.is_empty() || colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "format ~? only supports the at-sign modifier".to_string(),
            span: None,
        });
    }
    let nested_control = format_argument("~?", state.arguments, state.argument_index)?;
    let nested_control = match nested_control {
        Value::String(value) => value,
        value => return Err(type_error("format", "a string for ~?", value)),
    };
    if at_sign_modifier {
        let nested_characters = nested_control.chars().collect::<Vec<_>>();
        let (formatted, consumed, termination) = format_control_characters(
            &nested_characters,
            &state.arguments[*state.argument_index..],
            false,
        )?;
        state.output.push_str(&formatted);
        *state.argument_index += consumed;
        return Ok(termination);
    }
    let nested_arguments = format_argument("~?", state.arguments, state.argument_index)?;
    let nested_arguments = nested_arguments
        .list_items()
        .ok_or_else(|| type_error("format", "a list of arguments for ~?", nested_arguments))?;
    state
        .output
        .push_str(&format_control(nested_control, &nested_arguments)?);
    Ok(None)
}

pub(super) fn format_value_directive(
    directive: char,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    match directive {
        'A' => format_a_directive(
            arguments,
            argument_index,
            parameters,
            colon_modifier,
            at_sign_modifier,
        ),
        'S' => format_s_directive(arguments, argument_index, parameters, at_sign_modifier),
        _ => unreachable!("format value directive dispatch"),
    }
}

pub(super) fn format_radix_output(
    output: &mut String,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<(), RuntimeError> {
    let argument = format_argument("~R", arguments, argument_index)?;
    let integer = integer_argument("format", argument)?;
    output.push_str(&format_radix_directive(
        integer,
        parameters,
        colon_modifier,
        at_sign_modifier,
    )?);
    Ok(())
}

pub(super) fn format_tab_output(
    output: &mut String,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<(), RuntimeError> {
    let column = format_parameter_count(parameters, 0, 1)?;
    let increment = format_parameter_count(parameters, 1, 1)?;
    if colon_modifier {
        return Ok(());
    }
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
    output.extend(std::iter::repeat_n(' ', spaces));
    Ok(())
}

pub(super) fn format_write_output(
    output: &mut String,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
) -> Result<(), RuntimeError> {
    if !parameters.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format ~W does not accept parameters".to_string(),
            span: None,
        });
    }
    let argument = format_argument("~W", arguments, argument_index)?;
    output.push_str(&printed_value(argument, true));
    Ok(())
}

pub(super) fn format_simple_directive(
    directive: char,
    output: &mut String,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<bool, RuntimeError> {
    match directive {
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
                format_argument("~P", arguments, argument_index)?
            };
            let value = integer_argument("format", argument)?;
            if at_sign_modifier {
                output.push_str(if value == 1 { "y" } else { "ies" });
            } else if value != 1 {
                output.push('s');
            }
            Ok(true)
        }
        'C' => {
            let argument = format_argument("~C", arguments, argument_index)?;
            let Value::Character(character) = argument else {
                return Err(type_error("format", "a character for ~C", argument));
            };
            output.push_str(&format_character_directive(
                *character,
                colon_modifier,
                at_sign_modifier,
            ));
            Ok(true)
        }
        '%' | '&' | '|' | '~' => {
            let count = format_parameter_count(parameters, 0, 1)?;
            for repetition in 0..count {
                match directive {
                    '%' if repetition > 0 || (!output.is_empty() && !output.ends_with('\n')) => {
                        output.push('\n');
                    }
                    '&' if repetition == 0 => {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    '&' => output.push('\n'),
                    '|' => output.push('\x0c'),
                    '~' => output.push('~'),
                    _ => {}
                }
            }
            Ok(true)
        }
        '_' => {
            if !parameters.is_empty() {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~_ does not accept parameters".to_string(),
                    span: None,
                });
            }
            Ok(true)
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
            let _ = format_parameter_count(parameters, 0, 0)?;
            Ok(true)
        }
        '*' => {
            let count = format_parameter_count(parameters, 0, 1)?;
            *argument_index = argument_index.saturating_add(count).min(arguments.len());
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn format_numeric_directive(
    directive: char,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if matches!(directive, 'D' | 'B' | 'O' | 'X') {
        let argument = format_argument("format integer directive", arguments, argument_index)?;
        let integer = integer_argument("format", argument)?;
        let radix = match directive {
            'D' => 10,
            'B' => 2,
            'O' => 8,
            'X' => 16,
            _ => unreachable!(),
        };
        return format_integer_directive(
            integer,
            radix,
            parameters,
            colon_modifier,
            at_sign_modifier,
        );
    }

    let argument = format_argument("format number directive", arguments, argument_index)?;
    let value = number_argument("format", argument)?.as_float();
    match directive {
        'F' => format_fixed_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        'G' => format_general_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        'E' => {
            format_exponential_float_directive(value, parameters, colon_modifier, at_sign_modifier)
        }
        '$' => format_dollar_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        _ => unreachable!(),
    }
}

pub(super) fn format_a_directive(
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let argument = format_argument("~A", arguments, argument_index)?;
    let mut formatted = String::new();
    if colon_modifier && matches!(argument, Value::Nil) {
        formatted.push_str("()");
    } else {
        append_aesthetic(&mut formatted, argument);
    }
    format_text_field(&formatted, parameters, at_sign_modifier)
}

pub(super) fn format_s_directive(
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let argument = format_argument("~S", arguments, argument_index)?;
    format_text_field(&argument.to_string(), parameters, at_sign_modifier)
}

pub(super) fn format_justification_clauses(body: &[char]) -> Result<Vec<&[char]>, RuntimeError> {
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

pub(super) fn format_justification(
    clauses: &[&[char]],
    arguments: &[Value],
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
    colon_iteration_last: bool,
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

    let (pieces, argument_index) =
        format_justification_pieces(clauses, arguments, colon_iteration_last)?;

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
    let gap_count = usize::from(leading_gap)
        .saturating_add(between_count)
        .saturating_add(usize::from(trailing_gap));
    let distributed_padding = total_padding.saturating_sub(base_between_padding);
    let base_padding = distributed_padding.checked_div(gap_count).unwrap_or(0);
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
        output.extend(std::iter::repeat_n(pad_character, count));
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

pub(super) fn format_case_conversion(
    text: &str,
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> String {
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

pub(super) fn format_escape_upward(
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

pub(super) fn format_iteration(
    body: &[char],
    arguments: &[Value],
    colon_modifier: bool,
    limit: Option<usize>,
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut repetitions = 0;
    while argument_index < arguments.len() && limit.is_none_or(|limit| repetitions < limit) {
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
            )?;
            output.push_str(&formatted);
            (consumed, termination)
        } else {
            let (formatted, consumed, termination) =
                format_control_characters(body, &arguments[argument_index..], false)?;
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

pub(super) fn format_integer_directive(
    value: i64,
    radix: u32,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let padding_character = format_parameter_character(parameters, 1, ' ')?;
    let comma_character = format_parameter_character(parameters, 2, ',')?;
    let comma_interval = format_parameter_count(parameters, 3, 3)?;
    if colon_modifier && comma_interval == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format comma interval must be positive".to_string(),
            span: None,
        });
    }

    let mut digits = format_unsigned_integer(value.unsigned_abs(), radix);
    if colon_modifier {
        digits = format_grouped_digits(&digits, comma_character, comma_interval);
    }
    let mut formatted = String::new();
    if value < 0 {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);
    let padding = minimum_column.saturating_sub(formatted.chars().count());
    let mut result = String::new();
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    Ok(result)
}

pub(super) fn format_fixed_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~F".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let fractional_digits = match parameters
        .get(1)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let scale = format_parameter_number(parameters, 2, 0)?;
    let scale = i32::try_from(scale).map_err(|_| RuntimeError::InvalidForm {
        message: "format scale factor is out of range".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(3)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 3 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 4, ' ')?;
    let scaled = value * 10_f64.powi(scale);
    let negative = scaled.is_sign_negative();
    let magnitude = scaled.abs();
    let mut digits = fractional_digits.map_or_else(
        || {
            let mut digits = magnitude.to_string();
            if !digits.contains('.') && !digits.contains('e') && !digits.contains('E') {
                digits.push_str(".0");
            }
            digits
        },
        |fractional_digits| {
            let mut digits = format!("{magnitude:.fractional_digits$}");
            if fractional_digits == 0 {
                digits.push('.');
            }
            digits
        },
    );
    if let Some(fractional_digits) = fractional_digits
        && minimum_column == fractional_digits.saturating_add(1)
        && digits.starts_with("0.")
    {
        digits.remove(0);
    }

    let mut formatted = String::new();
    if negative {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);

    let width = formatted.chars().count();
    if minimum_column > 0 && width > minimum_column {
        if let Some(overflow_character) = overflow_character {
            return Ok(std::iter::repeat_n(overflow_character, minimum_column).collect());
        }
        return Ok(formatted);
    }
    let padding = minimum_column.saturating_sub(width);
    let mut result = String::new();
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    Ok(result)
}

pub(super) fn format_dollar_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let fractional_digits = format_parameter_count(parameters, 0, 2)?;
    let minimum_integer_digits = format_parameter_count(parameters, 1, 1)?;
    let minimum_column = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;

    let negative = value.is_sign_negative();
    let magnitude = value.abs();
    let mut digits = format!("{magnitude:.fractional_digits$}");
    if fractional_digits == 0 {
        digits.push('.');
    }
    let (integer_part, fractional_part) =
        digits
            .split_once('.')
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format ~$ could not produce a fixed-point number".to_string(),
                span: None,
            })?;

    let mut numeric = String::new();
    numeric.extend(std::iter::repeat_n(
        '0',
        minimum_integer_digits.saturating_sub(integer_part.chars().count()),
    ));
    numeric.push_str(integer_part);
    numeric.push('.');
    numeric.push_str(fractional_part);

    let sign = if negative {
        Some('-')
    } else if at_sign_modifier {
        Some('+')
    } else {
        None
    };
    let sign_width = usize::from(sign.is_some());
    let padding = minimum_column.saturating_sub(sign_width + numeric.chars().count());
    let mut result = String::new();
    if colon_modifier {
        if let Some(sign) = sign {
            result.push(sign);
        }
        result.extend(std::iter::repeat_n(padding_character, padding));
    } else {
        result.extend(std::iter::repeat_n(padding_character, padding));
        if let Some(sign) = sign {
            result.push(sign);
        }
    }
    result.push_str(&numeric);
    Ok(result)
}

pub(super) fn format_exponential_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~E".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let (requested_fractional_digits, requested_exponent_digits) =
        exponential_digit_parameters(parameters)?;
    let scale = i32::try_from(format_parameter_number(parameters, 3, 1)?).map_err(|_| {
        RuntimeError::InvalidForm {
            message: "format scale factor is out of range".to_string(),
            span: None,
        }
    })?;
    if let Some(fractional_digits) = requested_fractional_digits {
        let invalid_positive_scale = scale > 0
            && usize::try_from(scale)
                .is_ok_and(|scale| scale >= fractional_digits.saturating_add(2));
        let invalid_negative_scale = scale < 0
            && usize::try_from(scale.unsigned_abs()).is_ok_and(|scale| scale >= fractional_digits);
        if invalid_positive_scale || invalid_negative_scale {
            return Err(RuntimeError::InvalidForm {
                message: "format scale factor is incompatible with fractional digit count"
                    .to_string(),
                span: None,
            });
        }
    }
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let minimum = match scale.cmp(&0) {
            std::cmp::Ordering::Greater => usize::try_from(scale)
                .unwrap_or(usize::MAX)
                .saturating_sub(1),
            std::cmp::Ordering::Less => usize::try_from(scale.unsigned_abs())
                .unwrap_or(usize::MAX)
                .saturating_add(1),
            std::cmp::Ordering::Equal => 0,
        };
        6.max(minimum)
    });
    let significant_digits = match scale.cmp(&0) {
        std::cmp::Ordering::Greater => fractional_digits.checked_add(1),
        std::cmp::Ordering::Equal => Some(fractional_digits.max(1)),
        std::cmp::Ordering::Less => fractional_digits
            .checked_sub(usize::try_from(scale.unsigned_abs()).unwrap_or(usize::MAX)),
    }
    .filter(|value| *value > 0)
    .ok_or_else(|| RuntimeError::InvalidForm {
        message: "format scale factor leaves no significant digits".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(4)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 4 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 5, ' ')?;
    let exponent_character = format_parameter_character(parameters, 6, 'E')?;
    if !value.is_finite() {
        return Ok(format_non_finite_exponential(
            value,
            at_sign_modifier,
            minimum_column,
            overflow_character,
            padding_character,
        ));
    }
    let formatted = format_exponential_finite(
        value,
        ExponentialFiniteOptions {
            significant_digits,
            fractional_digits,
            trim_fractional_zeroes: requested_fractional_digits.is_none(),
            scale,
            requested_exponent_digits,
            exponent_character,
            at_sign_modifier,
        },
    )?;
    Ok(apply_exponential_field(
        formatted,
        minimum_column,
        overflow_character,
        padding_character,
    ))
}

#[cfg(test)]
#[path = "builtin_format_tests.rs"]
mod format_tests;

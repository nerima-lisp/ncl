
fn format_control_characters(
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
                | '_'
                | '('
                | '*'
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
                output.push_str(&format_text_field(
                    argument.to_string(),
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
                for repetition in 0..count {
                    if repetition == 0 && (output.is_empty() || output.ends_with('\n')) {
                        continue;
                    }
                    output.push('\n');
                }
            }
            '&' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for repetition in 0..count {
                    if repetition == 0 {
                        if !output.is_empty() && !output.ends_with('\n') {
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
            '\n' => {
                while matches!(
                    characters.get(character_index),
                    Some(character) if character.is_whitespace()
                ) {
                    character_index += 1;
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
                let _ = format_parameter_count(&parameters, 0, 0)?;
            }
            '*' => {
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~* does not support using colon and at-sign together"
                            .to_string(),
                        span: None,
                    });
                }
                if colon_modifier {
                    let count = format_parameter_count(&parameters, 0, 1)?;
                    argument_index = argument_index.checked_sub(count).ok_or_else(|| {
                        RuntimeError::InvalidForm {
                            message: "format ~:* has no previous argument".to_string(),
                            span: None,
                        }
                    })?;
                } else if at_sign_modifier {
                    let count = format_parameter_count(&parameters, 0, 0)?;
                    argument_index = count.min(arguments.len());
                } else {
                    let count = format_parameter_count(&parameters, 0, 1)?;
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
                    output.push_str(&format_control(nested_control, &nested_arguments)?);
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
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                } else {
                    let list = format_argument("~{", arguments, &mut argument_index)?;
                    let list = list
                        .list_items()
                        .ok_or_else(|| type_error("format", "a list for ~{", list))?;
                    let (formatted, _) = format_iteration(body, &list, colon_modifier, limit)?;
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
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
                    }
                } else if !colon_modifier
                    && !at_sign_modifier
                    && let Some((clause, _)) = clauses.iter().find(|(_, default)| *default)
                {
                    let (formatted, consumed, termination) = format_control_characters(
                        clause,
                        &arguments[argument_index..],
                        colon_iteration_last,
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
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
                    output.extend(std::iter::repeat_n(' ', spaces));
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


fn format_justification(
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

    let mut pieces = Vec::new();
    let mut argument_index = 0;
    for clause in clauses {
        let (formatted, consumed, termination) =
            format_control_characters(clause, &arguments[argument_index..], colon_iteration_last)?;
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
    let base_padding = distributed_padding
        .checked_div(gap_count)
        .unwrap_or_default();
    let remainder = distributed_padding
        .checked_rem(gap_count)
        .unwrap_or_default();
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
        formatted.extend(std::iter::repeat_n(padding_character, padding));
        formatted.push_str(&text);
    } else {
        formatted.push_str(&text);
        formatted.extend(std::iter::repeat_n(padding_character, padding));
    }
    Ok(formatted)
}

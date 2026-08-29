#[allow(clippy::wildcard_imports)]
use super::*;

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

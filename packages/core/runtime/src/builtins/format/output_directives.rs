#[allow(clippy::wildcard_imports)]
use super::*;

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
    let current_column = output
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .count();
    if colon_modifier && current_column >= column {
        return Ok(());
    }
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

#[allow(clippy::wildcard_imports)]
use super::*;

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

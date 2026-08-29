use super::*;

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

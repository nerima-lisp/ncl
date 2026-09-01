use crate::builtins::format::model::FormatDirective;
use crate::builtins::format::parser::{format_directive_prefix, parse_format_parameters};
use crate::{RuntimeError, Value};

pub(in crate::builtins::format) fn parse_format_directive(
    characters: &[char],
    character_index: &mut usize,
    arguments: &[Value],
    argument_index: &mut usize,
) -> Result<FormatDirective, RuntimeError> {
    let parameters =
        parse_format_parameters(characters, character_index, arguments, argument_index)?;
    let (directive_index, colon_modifier, at_sign_modifier) =
        format_directive_prefix(characters, *character_index)?;
    let directive = characters
        .get(directive_index)
        .copied()
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "format control ends after a tilde".to_string(),
            span: None,
        })?
        .to_ascii_uppercase();
    *character_index = directive_index + 1;
    if directive == '\n' {
        while characters
            .get(*character_index)
            .is_some_and(|character| matches!(character, ' ' | '\t' | '\n' | '\r'))
        {
            *character_index += 1;
        }
    }
    let supports_modifiers = matches!(
        directive,
        '{' | '['
            | '*'
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
            | '\n'
    );
    if (colon_modifier || at_sign_modifier) && !supports_modifiers {
        return Err(RuntimeError::InvalidForm {
            message: format!("unsupported format modifier before ~{directive}"),
            span: None,
        });
    }
    Ok(FormatDirective {
        parameters,
        directive,
        colon_modifier,
        at_sign_modifier,
    })
}
